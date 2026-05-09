use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use mcp_db::LinkedGitHubAccountRepository;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{db_error, internal_error};
use crate::extractors::AuthUser;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubRepo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub private: bool,
    pub html_url: String,
    pub default_branch: String,
    pub updated_at: String,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRepoResponse {
    id: i64,
    name: String,
    full_name: String,
    description: Option<String>,
    private: bool,
    html_url: String,
    default_branch: String,
    updated_at: String,
    language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReposQuery {
    /// Optional account ID to fetch repositories from a specific linked GitHub account
    pub account_id: Option<Uuid>,
}

pub async fn list_repositories(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(params): Query<ReposQuery>,
) -> Result<Json<Vec<GitHubRepo>>, (StatusCode, String)> {
    // Get the GitHub access token from linked accounts
    let linked_account = if let Some(account_id) = params.account_id {
        // Use specific account
        LinkedGitHubAccountRepository::get_with_token(&state.db, account_id, auth_user.user_id)
            .await
            .map_err(db_error)?
    } else {
        // Use primary or any linked account
        LinkedGitHubAccountRepository::get_any_with_token(&state.db, auth_user.user_id)
            .await
            .map_err(db_error)?
    };

    // If no linked account found, return empty array (not an error)
    let linked_account = match linked_account {
        Some(account) => account,
        None => {
            tracing::info!(
                "No linked GitHub account for user {}",
                auth_user.user_id
            );
            return Ok(Json(vec![]));
        }
    };

    // Decrypt GitHub access token
    let access_token = state
        .crypto
        .decrypt_string(&linked_account.access_token_encrypted, &linked_account.access_token_nonce)
        .map_err(|e| internal_error("Token decryption failed", e))?;

    // Fetch repositories from GitHub
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/user/repos")
        .query(&[("sort", "updated"), ("per_page", "100")])
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "MCP-Cloud/1.0")
        .send()
        .await
        .map_err(db_error)?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        tracing::error!("GitHub API error: {} - {}", status, body);
        return Err((
            StatusCode::BAD_REQUEST,
            "Failed to fetch repositories. Please re-authenticate.".to_string(),
        ));
    }

    let repos: Vec<GitHubRepoResponse> = response
        .json()
        .await
        .map_err(db_error)?;

    let result: Vec<GitHubRepo> = repos
        .into_iter()
        .map(|r| GitHubRepo {
            id: r.id,
            name: r.name,
            full_name: r.full_name,
            description: r.description,
            private: r.private,
            html_url: r.html_url,
            default_branch: r.default_branch,
            updated_at: r.updated_at,
            language: r.language,
        })
        .collect();

    Ok(Json(result))
}
