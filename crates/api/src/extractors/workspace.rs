use mcp_common::types::WorkspaceRole;
use mcp_db::{WorkspaceMember, WorkspaceRepository};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// Helper functions for workspace membership validation.
/// These reduce code duplication across route handlers.

/// Validates that a user is a member of the workspace.
/// Returns the member record if successful.
pub async fn require_member(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<WorkspaceMember, AppError> {
    WorkspaceRepository::get_member(pool, workspace_id, user_id)
        .await?
        .ok_or_else(|| AppError::forbidden("Not a member of this workspace"))
}

/// Validates that a user is at least an admin (admin or owner) of the workspace.
/// Returns the member record if successful.
pub async fn require_admin(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<WorkspaceMember, AppError> {
    let member = require_member(pool, workspace_id, user_id).await?;
    let role = member.role();

    if role != WorkspaceRole::Admin && role != WorkspaceRole::Owner {
        return Err(AppError::forbidden("Admin access required"));
    }

    Ok(member)
}

/// Validates that a user can write to the workspace (member, admin, or owner).
/// Viewers are excluded.
/// Returns the member record if successful.
pub async fn require_write_access(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<WorkspaceMember, AppError> {
    let member = require_member(pool, workspace_id, user_id).await?;

    if member.role() == WorkspaceRole::Viewer {
        return Err(AppError::forbidden("Write access required"));
    }

    Ok(member)
}
