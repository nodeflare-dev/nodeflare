//! Container management for MCP Cloud
//!
//! Provides abstraction over:
//! - Docker (local development)
//! - Fly.io Machines (production)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub image: String,
    pub env: HashMap<String, String>,
    pub port: u16,
    pub memory_mb: u32,
    pub cpu_shares: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub status: ContainerStatus,
    pub endpoint_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerStatus {
    Creating,
    Running,
    Stopped,
    Failed,
}

#[async_trait::async_trait]
pub trait ContainerRuntime: Send + Sync {
    async fn create(&self, name: &str, config: ContainerConfig) -> Result<Container>;
    async fn start(&self, id: &str) -> Result<()>;
    async fn stop(&self, id: &str) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn status(&self, id: &str) -> Result<ContainerStatus>;
    async fn logs(&self, id: &str, tail: usize) -> Result<String>;
}

pub mod docker;
pub mod flyio;

pub use docker::DockerRuntime;
pub use flyio::{
    AppMetrics, ExecResponse, FlyioRuntime, MetricDataPoint, WireGuardConfig, WireGuardPeerInfo,
};
