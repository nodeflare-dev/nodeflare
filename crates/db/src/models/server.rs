use chrono::{DateTime, Utc};
use mcp_common::types::{AccessMode, Runtime, ServerStatus, Transport, Visibility};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct McpServer {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub github_repo: String,
    pub github_branch: String,
    pub github_installation_id: Option<i64>,
    pub runtime: String,
    pub visibility: String,
    pub access_mode: String,
    pub transport: String,
    pub status: String,
    pub endpoint_url: Option<String>,
    pub rate_limit_per_minute: Option<i32>,
    pub region: String,
    pub root_directory: String,
    pub mcp_path: String,
    /// Custom entry command for the MCP server (e.g., "python server.py", "uv run mcp-server")
    /// If None, auto-detect based on project structure
    pub entry_command: Option<String>,
    /// When false, skip NodeFlare authentication layer (for servers that handle their own auth)
    pub auth_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl McpServer {
    pub fn runtime(&self) -> Runtime {
        match self.runtime.as_str() {
            "node" => Runtime::Node,
            "python" => Runtime::Python,
            "go" => Runtime::Go,
            "rust" => Runtime::Rust,
            "docker" => Runtime::Docker,
            _ => Runtime::Node,
        }
    }

    pub fn visibility(&self) -> Visibility {
        match self.visibility.as_str() {
            "team" => Visibility::Team,
            "public" => Visibility::Public,
            _ => Visibility::Private,
        }
    }

    pub fn access_mode(&self) -> AccessMode {
        match self.access_mode.as_str() {
            "vpn_only" => AccessMode::VpnOnly,
            _ => AccessMode::Public,
        }
    }

    pub fn status(&self) -> ServerStatus {
        match self.status.as_str() {
            "building" => ServerStatus::Building,
            "deploying" => ServerStatus::Deploying,
            "running" => ServerStatus::Running,
            "failed" => ServerStatus::Failed,
            "stopped" => ServerStatus::Stopped,
            _ => ServerStatus::Inactive,
        }
    }

    pub fn transport(&self) -> Transport {
        match self.transport.as_str() {
            "stdio" => Transport::Stdio,
            _ => Transport::Sse,
        }
    }

    pub fn is_running(&self) -> bool {
        self.status == "running"
    }
}

#[derive(Debug, Clone)]
pub struct CreateServer {
    pub workspace_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub github_repo: String,
    pub github_branch: String,
    pub github_installation_id: Option<i64>,
    pub runtime: Runtime,
    pub visibility: Visibility,
    pub access_mode: AccessMode,
    pub transport: Transport,
    pub region: String,
    pub root_directory: String,
    pub mcp_path: String,
    pub entry_command: Option<String>,
    pub auth_enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateServer {
    pub name: Option<String>,
    pub description: Option<String>,
    pub github_branch: Option<String>,
    pub visibility: Option<Visibility>,
    pub access_mode: Option<AccessMode>,
    pub status: Option<ServerStatus>,
    pub endpoint_url: Option<String>,
    pub region: Option<String>,
    pub root_directory: Option<String>,
    pub mcp_path: Option<String>,
    pub entry_command: Option<String>,
    pub auth_enabled: Option<bool>,
}
