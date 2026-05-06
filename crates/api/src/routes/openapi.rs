use axum::{routing::get, Router};
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "MCP Cloud API",
        version = "1.0.0",
        description = "API for MCP Cloud - Deploy and manage MCP servers"
    ),
    servers(
        (url = "/api/v1", description = "API v1")
    ),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "workspaces", description = "Workspace management"),
        (name = "servers", description = "MCP server management"),
        (name = "tools", description = "Tool management"),
        (name = "deployments", description = "Deployment management"),
        (name = "access-tokens", description = "Access token management"),
        (name = "secrets", description = "Secret management"),
        (name = "logs", description = "Request logs and analytics")
    )
)]
pub struct ApiDoc;

pub fn openapi_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/openapi.json", get(openapi_json))
        .merge(Scalar::with_url("/docs", ApiDoc::openapi()))
}

async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(ApiDoc::openapi())
}
