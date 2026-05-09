use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use fred::interfaces::KeysInterface;
use mcp_auth::GitHubOAuth;
use mcp_db::{CreateLinkedGitHubAccount, LinkedGitHubAccount, LinkedGitHubAccountRepository};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{db_error, internal_error};
use crate::extractors::AuthUser;
use crate::state::AppState;

const CSRF_TOKEN_PREFIX: &str = "csrf:github_link:";
const CSRF_TOKEN_TTL_SECS: i64 = 600; // 10 minutes

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct LinkedAccountResponse {
    pub id: Uuid,
    pub github_id: i64,
    pub github_username: String,
    pub github_avatar_url: Option<String>,
    pub is_primary: bool,
    pub created_at: String,
}

impl From<LinkedGitHubAccount> for LinkedAccountResponse {
    fn from(account: LinkedGitHubAccount) -> Self {
        Self {
            id: account.id,
            github_id: account.github_id,
            github_username: account.github_username,
            github_avatar_url: account.github_avatar_url,
            is_primary: account.is_primary,
            created_at: account.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

// ============================================================================
// Query Parameters
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct LinkQuery {
    pub return_to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// List all linked GitHub accounts for the current user
pub async fn list_accounts(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<LinkedAccountResponse>>, (StatusCode, String)> {
    let accounts = LinkedGitHubAccountRepository::list_by_user(&state.db, auth_user.user_id)
        .await
        .map_err(db_error)?;

    let response: Vec<LinkedAccountResponse> = accounts.into_iter().map(Into::into).collect();

    Ok(Json(response))
}

/// Initiate GitHub OAuth flow to link a new account
pub async fn link_account(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(params): Query<LinkQuery>,
) -> Result<Response, (StatusCode, String)> {
    // Generate CSRF token
    let csrf_token = mcp_auth::jwt::generate_random_token(32)
        .map_err(|e| internal_error("Token generation failed", e))?;

    // Store in Redis with user_id and optional return_to
    let csrf_key = format!("{}{}", CSRF_TOKEN_PREFIX, csrf_token);
    let redis_value = format!(
        "{}|{}",
        auth_user.user_id,
        params.return_to.unwrap_or_default()
    );

    state
        .redis
        .set::<(), _, _>(
            &csrf_key,
            &redis_value,
            Some(fred::types::Expiration::EX(CSRF_TOKEN_TTL_SECS)),
            None,
            false,
        )
        .await
        .map_err(|e| internal_error("Redis set failed", e))?;

    // Build GitHub OAuth URL with repo scope for repository access
    let redirect_url = format!(
        "{}/api/v1/github/accounts/callback",
        if state.config.is_production() {
            format!("https://{}", state.config.server.host)
        } else {
            format!("http://{}:{}", state.config.server.host, state.config.server.port)
        }
    );

    let oauth = GitHubOAuth::new(
        &state.config.github.client_id,
        &state.config.github.client_secret,
        &redirect_url,
    );

    // Request repo scope for full repository access
    let scopes = "repo,user:email";
    let auth_url = oauth.get_authorization_url_with_state_and_scopes(&csrf_token, scopes);

    // Redirect to GitHub
    Ok(Redirect::temporary(&auth_url).into_response())
}

/// Handle GitHub OAuth callback for account linking
pub async fn link_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, (StatusCode, String)> {
    // Handle OAuth errors
    if let Some(error) = &query.error {
        let error_url = format!(
            "{}/dashboard/settings/github?error={}",
            state.config.server.frontend_url,
            urlencoding::encode(error)
        );
        return Ok(Redirect::temporary(&error_url).into_response());
    }

    // Get code and state
    let code = query.code.as_ref().ok_or_else(|| {
        (StatusCode::BAD_REQUEST, "Missing authorization code".to_string())
    })?;

    let csrf_state = query.state.as_ref().ok_or_else(|| {
        (StatusCode::BAD_REQUEST, "Missing state parameter".to_string())
    })?;

    // Validate CSRF token and get user_id
    let csrf_key = format!("{}{}", CSRF_TOKEN_PREFIX, csrf_state);
    let csrf_value: Option<String> = state
        .redis
        .get(&csrf_key)
        .await
        .map_err(|e| internal_error("Redis get failed", e))?;

    let csrf_value = csrf_value.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, "Invalid or expired state parameter".to_string())
    })?;

    // Delete used CSRF token
    let _ = state.redis.del::<(), _>(&csrf_key).await;

    // Parse user_id and return_to from Redis value
    let parts: Vec<&str> = csrf_value.splitn(2, '|').collect();
    let user_id = Uuid::parse_str(parts[0]).map_err(|_| {
        (StatusCode::BAD_REQUEST, "Invalid state data".to_string())
    })?;
    let return_to = parts.get(1).map(|s| s.to_string()).filter(|s| !s.is_empty());

    // Build redirect URL for token exchange
    let redirect_url = format!(
        "{}/api/v1/github/accounts/callback",
        if state.config.is_production() {
            format!("https://{}", state.config.server.host)
        } else {
            format!("http://{}:{}", state.config.server.host, state.config.server.port)
        }
    );

    let oauth = GitHubOAuth::new(
        &state.config.github.client_id,
        &state.config.github.client_secret,
        &redirect_url,
    );

    // Exchange code for access token
    let access_token = oauth
        .exchange_code(code)
        .await
        .map_err(|e| {
            tracing::error!("GitHub code exchange failed: {}", e);
            (StatusCode::BAD_REQUEST, "GitHub authentication failed".to_string())
        })?;

    // Get GitHub user info
    let github_user = oauth
        .get_user(&access_token)
        .await
        .map_err(|e| {
            tracing::error!("GitHub get user failed: {}", e);
            (StatusCode::BAD_REQUEST, "Failed to get GitHub user info".to_string())
        })?;

    // Check if this GitHub account is already linked to this user
    if let Some(_existing) = LinkedGitHubAccountRepository::find_by_github_id(
        &state.db,
        user_id,
        github_user.id,
    )
    .await
    .map_err(db_error)?
    {
        // Update the token for existing linked account
        let encrypted = state
            .crypto
            .encrypt_string(&access_token)
            .map_err(|e| internal_error("Token encryption failed", e))?;

        LinkedGitHubAccountRepository::update_token(
            &state.db,
            _existing.id,
            user_id,
            &encrypted.0,
            &encrypted.1,
            Some("repo,user:email"),
        )
        .await
        .map_err(db_error)?;

        tracing::info!(
            "Updated GitHub account token: user={}, github_user={}",
            user_id,
            github_user.login
        );
    } else {
        // Create new linked account
        let encrypted = state
            .crypto
            .encrypt_string(&access_token)
            .map_err(|e| internal_error("Token encryption failed", e))?;

        LinkedGitHubAccountRepository::create(
            &state.db,
            CreateLinkedGitHubAccount {
                user_id,
                github_id: github_user.id,
                github_username: github_user.login.clone(),
                github_avatar_url: github_user.avatar_url.clone(),
                access_token_encrypted: encrypted.0,
                access_token_nonce: encrypted.1,
                scopes: Some("repo,user:email".to_string()),
            },
        )
        .await
        .map_err(db_error)?;

        tracing::info!(
            "Linked GitHub account: user={}, github_user={}",
            user_id,
            github_user.login
        );
    }

    // Redirect to frontend
    let redirect_url = if let Some(return_to) = return_to {
        format!("{}{}?success=github_linked", state.config.server.frontend_url, return_to)
    } else {
        format!("{}/dashboard/settings/github?success=github_linked", state.config.server.frontend_url)
    };

    Ok(Redirect::temporary(&redirect_url).into_response())
}

/// Delete a linked GitHub account
pub async fn unlink_account(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(account_id): Path<Uuid>,
) -> Result<Json<MessageResponse>, (StatusCode, String)> {
    let deleted = LinkedGitHubAccountRepository::delete(&state.db, account_id, auth_user.user_id)
        .await
        .map_err(db_error)?;

    if !deleted {
        return Err((StatusCode::NOT_FOUND, "Account not found".to_string()));
    }

    tracing::info!(
        "Unlinked GitHub account: user={}, account={}",
        auth_user.user_id,
        account_id
    );

    Ok(Json(MessageResponse {
        message: "GitHub account unlinked successfully".to_string(),
    }))
}

/// Set a linked account as primary
pub async fn set_primary(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(account_id): Path<Uuid>,
) -> Result<Json<MessageResponse>, (StatusCode, String)> {
    let updated = LinkedGitHubAccountRepository::set_primary(&state.db, account_id, auth_user.user_id)
        .await
        .map_err(db_error)?;

    if !updated {
        return Err((StatusCode::NOT_FOUND, "Account not found".to_string()));
    }

    tracing::info!(
        "Set primary GitHub account: user={}, account={}",
        auth_user.user_id,
        account_id
    );

    Ok(Json(MessageResponse {
        message: "Primary account updated successfully".to_string(),
    }))
}
