use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use mcp_db::{
    models::{CreateDeployWebhook, DeployWebhook, UpdateDeployWebhook},
    repositories::{DeployWebhookRepository, ServerRepository, WorkspaceRepository},
};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::Arc;
use url::Url;
use uuid::Uuid;

use crate::{error::AppError, extractors::AuthUser, state::AppState};

/// Validate webhook URL to prevent SSRF attacks
/// Blocks internal IPs, cloud metadata endpoints, and non-HTTPS URLs
fn validate_webhook_url(url_str: &str) -> Result<(), AppError> {
    // Must be HTTPS
    if !url_str.starts_with("https://") {
        return Err(AppError::bad_request("INVALID_URL", "Webhook URL must use HTTPS"));
    }

    // Parse the URL
    let url = Url::parse(url_str)
        .map_err(|_| AppError::bad_request("INVALID_URL", "Invalid URL format"))?;

    // Get the host
    let host = url.host_str()
        .ok_or_else(|| AppError::bad_request("INVALID_URL", "URL must have a host"))?;

    // Block localhost and common internal hostnames
    let blocked_hosts = [
        "localhost",
        "127.0.0.1",
        "0.0.0.0",
        "::1",
        "[::1]",
        "169.254.169.254",  // AWS/GCP metadata
        "metadata.google.internal",  // GCP metadata
        "metadata.internal",
        "kubernetes.default",
        "kubernetes.default.svc",
    ];

    let host_lower = host.to_lowercase();
    for blocked in &blocked_hosts {
        if host_lower == *blocked || host_lower.ends_with(&format!(".{}", blocked)) {
            return Err(AppError::bad_request(
                "BLOCKED_URL",
                "Webhook URL points to a blocked internal address",
            ));
        }
    }

    // Resolve hostname and check if it resolves to internal IP
    let port = url.port().unwrap_or(443);
    let socket_addr = format!("{}:{}", host, port);

    if let Ok(addrs) = socket_addr.to_socket_addrs() {
        for addr in addrs {
            if is_internal_ip(&addr.ip()) {
                return Err(AppError::bad_request(
                    "BLOCKED_URL",
                    "Webhook URL resolves to an internal IP address",
                ));
            }
        }
    }

    Ok(())
}

/// Check if an IP address is internal/private
fn is_internal_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            // Private ranges
            ipv4.is_private() ||
            // Loopback (127.0.0.0/8)
            ipv4.is_loopback() ||
            // Link-local (169.254.0.0/16)
            ipv4.is_link_local() ||
            // Broadcast
            ipv4.is_broadcast() ||
            // Documentation ranges
            ipv4.is_documentation() ||
            // Unspecified (0.0.0.0)
            ipv4.is_unspecified() ||
            // AWS/Cloud metadata range
            ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254
        }
        IpAddr::V6(ipv6) => {
            // Loopback (::1)
            ipv6.is_loopback() ||
            // Unspecified (::)
            ipv6.is_unspecified() ||
            // IPv4-mapped addresses - check the mapped IPv4
            if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                is_internal_ip(&IpAddr::V4(ipv4))
            } else {
                // Unique local addresses (fc00::/7)
                let segments = ipv6.segments();
                (segments[0] & 0xfe00) == 0xfc00 ||
                // Link-local (fe80::/10)
                (segments[0] & 0xffc0) == 0xfe80
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub id: String,
    pub name: String,
    pub webhook_url: String,
    pub webhook_type: String,
    pub events: Vec<String>,
    pub is_active: bool,
    pub last_triggered_at: Option<String>,
    pub last_status: Option<String>,
    pub created_at: String,
}

impl From<DeployWebhook> for WebhookResponse {
    fn from(w: DeployWebhook) -> Self {
        Self {
            id: w.id.to_string(),
            name: w.name,
            webhook_url: w.webhook_url,
            webhook_type: w.webhook_type,
            events: w.events,
            is_active: w.is_active,
            last_triggered_at: w.last_triggered_at.map(|t| t.to_rfc3339()),
            last_status: w.last_status,
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

/// List webhooks for a server
pub async fn list(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((workspace_id, server_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<WebhookResponse>>, AppError> {
    // Verify user has access to workspace
    WorkspaceRepository::get_member(&state.db, workspace_id, auth_user.user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get workspace member: {}", e);
            AppError::internal("Failed to verify access")
        })?
        .ok_or_else(|| AppError::forbidden("Access denied"))?;

    // Verify server belongs to workspace
    let server = ServerRepository::find_by_id(&state.db, server_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get server: {}", e);
            AppError::internal("Failed to get server")
        })?
        .ok_or_else(|| AppError::not_found("Server not found"))?;

    if server.workspace_id != workspace_id {
        return Err(AppError::not_found("Server not found"));
    }

    let webhooks = DeployWebhookRepository::list_by_server(&state.db, server_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list webhooks: {}", e);
            AppError::internal("Failed to list webhooks")
        })?;

    Ok(Json(webhooks.into_iter().map(WebhookResponse::from).collect()))
}

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub name: String,
    pub webhook_url: String,
    pub webhook_type: String,
    pub events: Vec<String>,
    pub secret: Option<String>,
}

/// Create a new webhook
pub async fn create(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((workspace_id, server_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CreateWebhookRequest>,
) -> Result<Json<WebhookResponse>, AppError> {
    // Verify user has access to workspace (admin or owner)
    let member = WorkspaceRepository::get_member(&state.db, workspace_id, auth_user.user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get workspace member: {}", e);
            AppError::internal("Failed to verify access")
        })?
        .ok_or_else(|| AppError::forbidden("Access denied"))?;

    if !matches!(member.role(), mcp_common::types::WorkspaceRole::Owner | mcp_common::types::WorkspaceRole::Admin) {
        return Err(AppError::forbidden("Only owners and admins can manage webhooks"));
    }

    // Verify server belongs to workspace
    let server = ServerRepository::find_by_id(&state.db, server_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get server: {}", e);
            AppError::internal("Failed to get server")
        })?
        .ok_or_else(|| AppError::not_found("Server not found"))?;

    if server.workspace_id != workspace_id {
        return Err(AppError::not_found("Server not found"));
    }

    // Validate webhook URL (SSRF protection)
    validate_webhook_url(&body.webhook_url)?;

    // Validate events
    let valid_events = ["deploy_success", "deploy_failure", "deploy_started"];
    for event in &body.events {
        if !valid_events.contains(&event.as_str()) {
            return Err(AppError::bad_request("INVALID_EVENT", &format!("Invalid event: {}", event)));
        }
    }

    // Encrypt the secret if provided
    let (encrypted_secret, secret_nonce) = if let Some(ref secret) = body.secret {
        let (enc, nonce) = state.crypto.encrypt_string(secret).map_err(|e| {
            tracing::error!("Failed to encrypt webhook secret: {}", e);
            AppError::internal("Failed to encrypt secret")
        })?;
        (Some(enc), Some(nonce))
    } else {
        (None, None)
    };

    let webhook = DeployWebhookRepository::create(
        &state.db,
        CreateDeployWebhook {
            server_id,
            name: body.name,
            webhook_url: body.webhook_url,
            webhook_type: body.webhook_type,
            events: body.events,
            encrypted_secret,
            secret_nonce,
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create webhook: {}", e);
        AppError::internal("Failed to create webhook")
    })?;

    Ok(Json(WebhookResponse::from(webhook)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebhookRequest {
    pub name: Option<String>,
    pub webhook_url: Option<String>,
    pub events: Option<Vec<String>>,
    pub secret: Option<String>,
    pub is_active: Option<bool>,
}

/// Update a webhook
pub async fn update(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((workspace_id, server_id, webhook_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<UpdateWebhookRequest>,
) -> Result<Json<WebhookResponse>, AppError> {
    // Verify user has access to workspace (admin or owner)
    let member = WorkspaceRepository::get_member(&state.db, workspace_id, auth_user.user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get workspace member: {}", e);
            AppError::internal("Failed to verify access")
        })?
        .ok_or_else(|| AppError::forbidden("Access denied"))?;

    if !matches!(member.role(), mcp_common::types::WorkspaceRole::Owner | mcp_common::types::WorkspaceRole::Admin) {
        return Err(AppError::forbidden("Only owners and admins can manage webhooks"));
    }

    // Verify webhook exists and belongs to the server
    let existing = DeployWebhookRepository::find_by_id(&state.db, webhook_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get webhook: {}", e);
            AppError::internal("Failed to get webhook")
        })?
        .ok_or_else(|| AppError::not_found("Webhook not found"))?;

    if existing.server_id != server_id {
        return Err(AppError::not_found("Webhook not found"));
    }

    // Validate webhook URL if provided (SSRF protection)
    if let Some(ref url) = body.webhook_url {
        validate_webhook_url(url)?;
    }

    // Validate events if provided
    if let Some(ref events) = body.events {
        let valid_events = ["deploy_success", "deploy_failure", "deploy_started"];
        for event in events {
            if !valid_events.contains(&event.as_str()) {
                return Err(AppError::bad_request("INVALID_EVENT", &format!("Invalid event: {}", event)));
            }
        }
    }

    // Encrypt the secret if provided
    let (encrypted_secret, secret_nonce) = if let Some(ref secret) = body.secret {
        let (enc, nonce) = state.crypto.encrypt_string(secret).map_err(|e| {
            tracing::error!("Failed to encrypt webhook secret: {}", e);
            AppError::internal("Failed to encrypt secret")
        })?;
        (Some(enc), Some(nonce))
    } else {
        (None, None)
    };

    let webhook = DeployWebhookRepository::update(
        &state.db,
        webhook_id,
        UpdateDeployWebhook {
            name: body.name,
            webhook_url: body.webhook_url,
            events: body.events,
            encrypted_secret,
            secret_nonce,
            is_active: body.is_active,
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to update webhook: {}", e);
        AppError::internal("Failed to update webhook")
    })?
    .ok_or_else(|| AppError::not_found("Webhook not found"))?;

    Ok(Json(WebhookResponse::from(webhook)))
}

/// Delete a webhook
pub async fn delete(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((workspace_id, server_id, webhook_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    // Verify user has access to workspace (admin or owner)
    let member = WorkspaceRepository::get_member(&state.db, workspace_id, auth_user.user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get workspace member: {}", e);
            AppError::internal("Failed to verify access")
        })?
        .ok_or_else(|| AppError::forbidden("Access denied"))?;

    if !matches!(member.role(), mcp_common::types::WorkspaceRole::Owner | mcp_common::types::WorkspaceRole::Admin) {
        return Err(AppError::forbidden("Only owners and admins can manage webhooks"));
    }

    // Verify webhook exists and belongs to the server
    let existing = DeployWebhookRepository::find_by_id(&state.db, webhook_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get webhook: {}", e);
            AppError::internal("Failed to get webhook")
        })?
        .ok_or_else(|| AppError::not_found("Webhook not found"))?;

    if existing.server_id != server_id {
        return Err(AppError::not_found("Webhook not found"));
    }

    DeployWebhookRepository::delete(&state.db, webhook_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete webhook: {}", e);
            AppError::internal("Failed to delete webhook")
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// Test a webhook by sending a test payload
pub async fn test(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((workspace_id, server_id, webhook_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify user has access to workspace
    WorkspaceRepository::get_member(&state.db, workspace_id, auth_user.user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get workspace member: {}", e);
            AppError::internal("Failed to verify access")
        })?
        .ok_or_else(|| AppError::forbidden("Access denied"))?;

    // Get webhook
    let webhook = DeployWebhookRepository::find_by_id(&state.db, webhook_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get webhook: {}", e);
            AppError::internal("Failed to get webhook")
        })?
        .ok_or_else(|| AppError::not_found("Webhook not found"))?;

    if webhook.server_id != server_id {
        return Err(AppError::not_found("Webhook not found"));
    }

    // Validate webhook URL before sending (SSRF protection)
    validate_webhook_url(&webhook.webhook_url)?;

    // Send test payload
    let test_payload = serde_json::json!({
        "event": "test",
        "server_id": server_id.to_string(),
        "webhook_id": webhook_id.to_string(),
        "message": "This is a test webhook notification from NodeFlare",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    // Create client with redirect policy disabled to prevent SSRF via redirects
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| AppError::internal("Failed to create HTTP client"))?;

    let response = client
        .post(&webhook.webhook_url)
        .json(&test_payload)
        .send()
        .await;

    match response {
        Ok(res) => {
            let status = res.status();
            DeployWebhookRepository::update_trigger_status(
                &state.db,
                webhook_id,
                if status.is_success() { "success" } else { "failure" },
            )
            .await
            .ok();

            Ok(Json(serde_json::json!({
                "success": status.is_success(),
                "status_code": status.as_u16(),
            })))
        }
        Err(e) => {
            tracing::warn!("Webhook test failed for {}: {}", webhook_id, e);
            DeployWebhookRepository::update_trigger_status(&state.db, webhook_id, "failure")
                .await
                .ok();

            // Provide user-friendly error messages without exposing internal details
            let error_message = if e.is_timeout() {
                "Connection timed out"
            } else if e.is_connect() {
                "Failed to connect to webhook URL"
            } else if e.is_request() {
                "Invalid request"
            } else {
                "Failed to send webhook"
            };

            Ok(Json(serde_json::json!({
                "success": false,
                "error": error_message,
            })))
        }
    }
}
