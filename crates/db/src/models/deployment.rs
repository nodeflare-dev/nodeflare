use chrono::{DateTime, Utc};
use mcp_common::types::DeploymentStatus;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Deployment {
    pub id: Uuid,
    pub server_id: Uuid,
    pub version: i32,
    pub commit_sha: String,
    pub status: String,
    pub build_logs: Option<String>,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub deployed_by: Option<Uuid>,
}

impl Deployment {
    pub fn status(&self) -> DeploymentStatus {
        match self.status.as_str() {
            "building" => DeploymentStatus::Building,
            "pushing" => DeploymentStatus::Pushing,
            "deploying" => DeploymentStatus::Deploying,
            "succeeded" => DeploymentStatus::Succeeded,
            "failed" => DeploymentStatus::Failed,
            "cancelled" => DeploymentStatus::Cancelled,
            _ => DeploymentStatus::Pending,
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(
            self.status.as_str(),
            "succeeded" | "failed" | "cancelled"
        )
    }
}

#[derive(Debug, Clone)]
pub struct CreateDeployment {
    pub server_id: Uuid,
    pub commit_sha: String,
    pub deployed_by: Option<Uuid>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateDeployment {
    pub status: Option<DeploymentStatus>,
    pub build_logs: Option<String>,
    pub error_message: Option<String>,
    pub finished_at: Option<DateTime<Utc>>,
}
