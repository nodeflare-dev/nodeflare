use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use mcp_db::UserRepository;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::db_error;
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

pub async fn list_repositories(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<GitHubRepo>>, (StatusCode, String)> {
    // Get user with encrypted GitHub token
    let user = UserRepository::get_with_token(&state.db, auth_user.user_id)
        .await
        .map_err(db_error)?
        .ok_or((StatusCode::NOT_FOUND, "User not found".to_string()))?;

    // Decrypt GitHub access token
    let (encrypted_token, nonce) = match (
        user.github_access_token_encrypted,
        user.github_access_token_nonce,
    ) {
        (Some(token), Some(nonce)) => (token, nonce),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "GitHub token not found. Please re-authenticate.".to_string(),
            ))
        }
    };

    let access_token = state
        .crypto
        .decrypt_string(&encrypted_token, &nonce)
        .map_err(db_error)?;

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
