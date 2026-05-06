use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ErrorHint {
    pub id: Uuid,
    pub keywords: Vec<String>,
    pub hint_message: String,
    pub priority: i32,
    pub category: String,
    pub locale: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateErrorHint {
    pub keywords: Vec<String>,
    pub hint_message: String,
    pub priority: Option<i32>,
    pub category: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateErrorHint {
    pub keywords: Option<Vec<String>>,
    pub hint_message: Option<String>,
    pub priority: Option<i32>,
    pub category: Option<String>,
    pub locale: Option<String>,
    pub is_active: Option<bool>,
}
