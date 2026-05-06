use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RequestLog {
    pub id: Uuid,
    pub server_id: Uuid,
    pub tool_name: Option<String>,
    pub api_key_id: Option<Uuid>,
    pub client_info: Option<serde_json::Value>,
    pub request_body: Option<serde_json::Value>,
    pub response_status: String,
    pub error_message: Option<String>,
    pub duration_ms: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateRequestLog {
    pub server_id: Uuid,
    pub tool_name: Option<String>,
    pub api_key_id: Option<Uuid>,
    pub client_info: Option<serde_json::Value>,
    pub request_body: Option<serde_json::Value>,
    pub response_status: String,
    pub error_message: Option<String>,
    pub duration_ms: i32,
}

/// RequestLog with total count from window function (for optimized queries)
#[derive(Debug, Clone, FromRow)]
pub struct RequestLogWithCount {
    pub id: Uuid,
    pub server_id: Uuid,
    pub tool_name: Option<String>,
    pub api_key_id: Option<Uuid>,
    pub client_info: Option<serde_json::Value>,
    pub request_body: Option<serde_json::Value>,
    pub response_status: String,
    pub error_message: Option<String>,
    pub duration_ms: i32,
    pub created_at: DateTime<Utc>,
    pub total_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RequestLogStats {
    pub total_requests: i64,
    pub success_count: i64,
    pub error_count: i64,
    pub avg_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ToolUsageStats {
    pub tool_name: String,
    pub call_count: i64,
    pub error_count: i64,
    pub avg_duration_ms: f64,
}

/// Summary stats for a server (used in batch stats endpoint)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServerStatsSummary {
    pub server_id: Uuid,
    pub total_requests: i64,
    pub error_count: i64,
}
