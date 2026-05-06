use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub github_id: i64,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserWithToken {
    pub id: Uuid,
    pub github_id: i64,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub is_admin: bool,
    pub github_access_token_encrypted: Option<Vec<u8>>,
    pub github_access_token_nonce: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateUser {
    pub github_id: i64,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateUser {
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}
