use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub async fn readiness_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<HealthResponse>, StatusCode> {
    // Check database connection
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Check Redis connection
    let _: String = fred::interfaces::ClientLike::ping(&state.redis)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok(Json(HealthResponse {
        status: "ready".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}
