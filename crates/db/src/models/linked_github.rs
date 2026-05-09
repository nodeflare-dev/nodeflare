use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Represents a linked GitHub account for a user
/// Users can link multiple GitHub accounts for repository access
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LinkedGitHubAccount {
    pub id: Uuid,
    pub user_id: Uuid,
    pub github_id: i64,
    pub github_username: String,
    pub github_avatar_url: Option<String>,
    pub scopes: Option<String>,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Linked GitHub account with encrypted token data for API access
#[derive(Debug, Clone, FromRow)]
pub struct LinkedGitHubAccountWithToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub github_id: i64,
    pub github_username: String,
    pub github_avatar_url: Option<String>,
    pub access_token_encrypted: Vec<u8>,
    pub access_token_nonce: Vec<u8>,
    pub scopes: Option<String>,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Data for creating a new linked GitHub account
#[derive(Debug, Clone)]
pub struct CreateLinkedGitHubAccount {
    pub user_id: Uuid,
    pub github_id: i64,
    pub github_username: String,
    pub github_avatar_url: Option<String>,
    pub access_token_encrypted: Vec<u8>,
    pub access_token_nonce: Vec<u8>,
    pub scopes: Option<String>,
}
