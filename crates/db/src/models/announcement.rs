use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Announcement {
    pub id: Uuid,
    pub title: String,
    pub content: Option<String>,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub announcement_type: String,
    pub is_active: bool,
    pub published_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAnnouncement {
    pub title: String,
    pub content: Option<String>,
    #[serde(rename = "type")]
    pub announcement_type: String,
    pub expires_at: Option<DateTime<Utc>>,
}
