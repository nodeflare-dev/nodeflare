use chrono::{DateTime, Utc};
use mcp_common::types::ToolPermissionLevel;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tool {
    pub id: Uuid,
    pub server_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
    pub enabled: bool,
    pub permission_level: String,
    pub rate_limit_per_minute: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Tool {
    pub fn permission_level(&self) -> ToolPermissionLevel {
        match self.permission_level.as_str() {
            "elevated" => ToolPermissionLevel::Elevated,
            "dangerous" => ToolPermissionLevel::Dangerous,
            _ => ToolPermissionLevel::Normal,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateTool {
    pub server_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateTool {
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub permission_level: Option<ToolPermissionLevel>,
    pub rate_limit_per_minute: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct UpsertTool {
    pub server_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
}
