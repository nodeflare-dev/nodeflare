use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserPreferences {
    pub user_id: Uuid,
    pub sidebar_order: sqlx::types::Json<Vec<String>>,
    pub locale: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserPreferences {
    pub sidebar_order: Option<Vec<String>>,
    pub locale: Option<String>,
}
