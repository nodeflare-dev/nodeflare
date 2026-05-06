//! Public Platform Statistics
//!
//! Provides public statistics about the platform for display on the landing page.
//! No authentication required.

use axum::{extract::State, http::StatusCode, Json};
use mcp_db::ServerRepository;
use serde::Serialize;
use std::sync::Arc;

use crate::state::AppState;

/// Platform statistics response
#[derive(Debug, Serialize)]
pub struct PlatformStatsResponse {
    /// Total number of MCP servers created
    pub total_servers: i64,
    /// Number of currently running servers
    pub running_servers: i64,
    /// Total number of workspaces
    pub total_workspaces: i64,
}

/// Get public platform statistics
///
/// This endpoint does not require authentication and can be called from the landing page.
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<PlatformStatsResponse>, (StatusCode, String)> {
    let stats = ServerRepository::get_platform_stats(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get platform stats: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get stats".to_string())
        })?;

    Ok(Json(PlatformStatsResponse {
        total_servers: stats.total_servers,
        running_servers: stats.running_servers,
        total_workspaces: stats.total_workspaces,
    }))
}
