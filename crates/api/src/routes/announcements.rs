use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use mcp_db::{models::{Announcement, CreateAnnouncement}, repositories::AnnouncementRepository, UserRepository};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{error::AppError, extractors::AuthUser, state::AppState};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    5
}

/// Helper function to verify admin privileges
async fn require_admin(state: &AppState, user_id: uuid::Uuid) -> Result<(), AppError> {
    let is_admin = UserRepository::is_admin(&state.db, user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check admin status: {}", e);
            AppError::internal("Failed to verify admin status")
        })?;

    if !is_admin {
        return Err(AppError::forbidden("Admin privileges required"));
    }

    Ok(())
}

/// List active announcements (public endpoint, no auth required)
pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<Announcement>>, AppError> {
    let announcements = AnnouncementRepository::list_active(&state.db, query.limit)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch announcements: {}", e);
            AppError::internal("Failed to fetch announcements")
        })?;

    Ok(Json(announcements))
}

/// List all announcements (admin only)
pub async fn list_all(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<Announcement>>, AppError> {
    // Verify admin privileges
    require_admin(&state, auth_user.user_id).await?;

    let announcements = AnnouncementRepository::list_all(&state.db, query.limit, 0)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch announcements: {}", e);
            AppError::internal("Failed to fetch announcements")
        })?;

    Ok(Json(announcements))
}

#[derive(Debug, Deserialize)]
pub struct CreateAnnouncementRequest {
    pub title: String,
    pub content: Option<String>,
    #[serde(rename = "type")]
    pub announcement_type: String,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Create a new announcement (admin only)
pub async fn create(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(body): Json<CreateAnnouncementRequest>,
) -> Result<Json<Announcement>, AppError> {
    // Verify admin privileges
    require_admin(&state, auth_user.user_id).await?;

    let announcement = AnnouncementRepository::create(
        &state.db,
        CreateAnnouncement {
            title: body.title,
            content: body.content,
            announcement_type: body.announcement_type,
            expires_at: body.expires_at,
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create announcement: {}", e);
        AppError::internal("Failed to create announcement")
    })?;

    Ok(Json(announcement))
}

#[derive(Debug, Deserialize)]
pub struct UpdateAnnouncementRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    #[serde(rename = "type")]
    pub announcement_type: Option<String>,
    pub is_active: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Update an announcement (admin only)
pub async fn update(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateAnnouncementRequest>,
) -> Result<Json<Announcement>, AppError> {
    // Verify admin privileges
    require_admin(&state, auth_user.user_id).await?;

    let announcement = AnnouncementRepository::update(
        &state.db,
        id,
        body.title,
        body.content,
        body.announcement_type,
        body.is_active,
        body.expires_at,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to update announcement: {}", e);
        AppError::internal("Failed to update announcement")
    })?
    .ok_or_else(|| AppError::not_found("Announcement not found"))?;

    Ok(Json(announcement))
}

/// Delete an announcement (admin only)
pub async fn delete(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    // Verify admin privileges
    require_admin(&state, auth_user.user_id).await?;

    let deleted = AnnouncementRepository::delete(&state.db, id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete announcement: {}", e);
            AppError::internal("Failed to delete announcement")
        })?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("Announcement not found"))
    }
}
