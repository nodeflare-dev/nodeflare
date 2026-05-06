use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Secret {
    pub id: Uuid,
    pub server_id: Uuid,
    pub key: String,
    /// SECURITY: encrypted_value should never be serialized to API responses
    #[serde(skip_serializing)]
    pub encrypted_value: Vec<u8>,
    /// SECURITY: nonce should never be serialized to API responses
    #[serde(skip_serializing)]
    pub nonce: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateSecret {
    pub server_id: Uuid,
    pub key: String,
    pub encrypted_value: Vec<u8>,
    pub nonce: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct UpdateSecret {
    pub encrypted_value: Vec<u8>,
    pub nonce: Vec<u8>,
}
