use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeployWebhook {
    pub id: Uuid,
    pub server_id: Uuid,
    pub name: String,
    pub webhook_url: String,
    pub webhook_type: String,
    pub events: Vec<String>,
    /// Deprecated: Use encrypted_secret instead. Kept for backwards compatibility during migration.
    #[serde(skip_serializing)]
    pub secret: Option<String>,
    /// AES-256-GCM encrypted webhook secret
    /// SECURITY: encrypted_secret should never be serialized to API responses
    #[serde(skip_serializing)]
    pub encrypted_secret: Option<Vec<u8>>,
    /// Nonce for AES-256-GCM encryption
    /// SECURITY: secret_nonce should never be serialized to API responses
    #[serde(skip_serializing)]
    pub secret_nonce: Option<Vec<u8>>,
    pub is_active: bool,
    pub last_triggered_at: Option<DateTime<Utc>>,
    pub last_status: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDeployWebhook {
    pub server_id: Uuid,
    pub name: String,
    pub webhook_url: String,
    pub webhook_type: String,
    pub events: Vec<String>,
    /// Encrypted webhook secret
    pub encrypted_secret: Option<Vec<u8>>,
    /// Nonce for encryption
    pub secret_nonce: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDeployWebhook {
    pub name: Option<String>,
    pub webhook_url: Option<String>,
    pub events: Option<Vec<String>>,
    /// Encrypted webhook secret
    pub encrypted_secret: Option<Vec<u8>>,
    /// Nonce for encryption
    pub secret_nonce: Option<Vec<u8>>,
    pub is_active: Option<bool>,
}
