use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegionStatus {
    Pending,
    Deploying,
    Running,
    Stopped,
    Failed,
}

impl RegionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RegionStatus::Pending => "pending",
            RegionStatus::Deploying => "deploying",
            RegionStatus::Running => "running",
            RegionStatus::Stopped => "stopped",
            RegionStatus::Failed => "failed",
        }
    }
}

impl From<&str> for RegionStatus {
    fn from(s: &str) -> Self {
        match s {
            "deploying" => RegionStatus::Deploying,
            "running" => RegionStatus::Running,
            "stopped" => RegionStatus::Stopped,
            "failed" => RegionStatus::Failed,
            _ => RegionStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServerRegion {
    pub id: Uuid,
    pub server_id: Uuid,
    pub region: String,
    pub is_primary: bool,
    pub machine_id: Option<String>,
    pub status: String,
    pub endpoint_url: Option<String>,
    pub cancel_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ServerRegion {
    pub fn status(&self) -> RegionStatus {
        RegionStatus::from(self.status.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct CreateServerRegion {
    pub server_id: Uuid,
    pub region: String,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateServerRegion {
    pub machine_id: Option<String>,
    pub status: Option<RegionStatus>,
    pub endpoint_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RegionUsage {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub server_id: Uuid,
    pub region: String,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub active_hours: i32,
    /// Memory-weighted active minutes (sum of memory_gb * sample_minutes). GB-hours = gb_minutes / 60.
    pub gb_minutes: f64,
    pub reported_to_stripe: bool,
    pub stripe_usage_record_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateRegionUsage {
    pub workspace_id: Uuid,
    pub server_id: Uuid,
    pub region: String,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
}
