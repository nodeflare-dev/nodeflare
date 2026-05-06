use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use mcp_db::{
    models::UpdateNotificationSettings,
    repositories::NotificationSettingsRepository,
};
use serde::Serialize;
use std::sync::Arc;

use crate::error::db_error;
use crate::extractors::AuthUser;
use crate::state::AppState;

#[derive(Serialize)]
pub struct NotificationSettingsResponse {
    pub email_deploy_success: bool,
    pub email_deploy_failure: bool,
    pub email_server_down: bool,
    pub email_weekly_report: bool,
}

pub async fn get_settings(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<NotificationSettingsResponse>, (StatusCode, String)> {
    let settings = NotificationSettingsRepository::get_or_create(&state.db, auth_user.user_id)
        .await
        .map_err(db_error)?;

    Ok(Json(NotificationSettingsResponse {
        email_deploy_success: settings.email_deploy_success,
        email_deploy_failure: settings.email_deploy_failure,
        email_server_down: settings.email_server_down,
        email_weekly_report: settings.email_weekly_report,
    }))
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(body): Json<UpdateNotificationSettings>,
) -> Result<Json<NotificationSettingsResponse>, (StatusCode, String)> {
    let settings = NotificationSettingsRepository::update(&state.db, auth_user.user_id, body)
        .await
        .map_err(db_error)?;

    Ok(Json(NotificationSettingsResponse {
        email_deploy_success: settings.email_deploy_success,
        email_deploy_failure: settings.email_deploy_failure,
        email_server_down: settings.email_server_down,
        email_weekly_report: settings.email_weekly_report,
    }))
}
