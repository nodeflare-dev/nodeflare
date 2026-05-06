use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NotificationSettings {
    pub user_id: Uuid,
    pub email_deploy_success: bool,
    pub email_deploy_failure: bool,
    pub email_server_down: bool,
    pub email_weekly_report: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateNotificationSettings {
    pub email_deploy_success: Option<bool>,
    pub email_deploy_failure: Option<bool>,
    pub email_server_down: Option<bool>,
    pub email_weekly_report: Option<bool>,
}
