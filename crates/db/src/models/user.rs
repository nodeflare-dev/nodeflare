use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Authentication provider used for registration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthProvider {
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "google")]
    Google,
    #[serde(rename = "email")]
    Email,
}

impl Default for AuthProvider {
    fn default() -> Self {
        Self::GitHub
    }
}

impl std::fmt::Display for AuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthProvider::GitHub => write!(f, "github"),
            AuthProvider::Google => write!(f, "google"),
            AuthProvider::Email => write!(f, "email"),
        }
    }
}

impl std::str::FromStr for AuthProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "github" => Ok(AuthProvider::GitHub),
            "google" => Ok(AuthProvider::Google),
            "email" => Ok(AuthProvider::Email),
            _ => Err(format!("Unknown auth provider: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub github_id: Option<i64>,
    pub google_id: Option<String>,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub is_admin: bool,
    pub email_verified: bool,
    pub auth_provider: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn get_auth_provider(&self) -> AuthProvider {
        self.auth_provider.parse().unwrap_or_default()
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct UserWithToken {
    pub id: Uuid,
    pub github_id: Option<i64>,
    pub google_id: Option<String>,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub is_admin: bool,
    pub email_verified: bool,
    pub auth_provider: String,
    pub password_hash: Option<String>,
    pub github_access_token_encrypted: Option<Vec<u8>>,
    pub github_access_token_nonce: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateUserFromGitHub {
    pub github_id: i64,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateUserFromGoogle {
    pub google_id: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateUserFromEmail {
    pub email: String,
    pub name: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateUser {
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

/// Email verification token model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EmailVerificationToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub token_type: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

/// Token type for email verification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    EmailVerification,
    PasswordReset,
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenType::EmailVerification => write!(f, "verification"),
            TokenType::PasswordReset => write!(f, "password_reset"),
        }
    }
}

impl std::str::FromStr for TokenType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "verification" => Ok(TokenType::EmailVerification),
            "password_reset" => Ok(TokenType::PasswordReset),
            _ => Err(format!("Unknown token type: {}", s)),
        }
    }
}
