use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use mcp_db::UserPreferencesRepository;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::db_error;
use crate::extractors::AuthUser;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct UserPreferencesResponse {
    pub sidebar_order: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesRequest {
    pub sidebar_order: Vec<String>,
}

/// Get user preferences
pub async fn get_preferences(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<UserPreferencesResponse>, (StatusCode, String)> {
    let prefs = UserPreferencesRepository::find_by_user_id(&state.db, auth_user.user_id)
        .await
        .map_err(db_error)?;

    let sidebar_order = prefs
        .map(|p| p.sidebar_order.0)
        .unwrap_or_else(|| vec![
            "overview".to_string(),
            "servers".to_string(),
            "accessTokens".to_string(),
            "team".to_string(),
            "logs".to_string(),
            "billing".to_string(),
            "settings".to_string(),
        ]);

    Ok(Json(UserPreferencesResponse { sidebar_order }))
}

/// Update user preferences
pub async fn update_preferences(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(body): Json<UpdatePreferencesRequest>,
) -> Result<Json<UserPreferencesResponse>, (StatusCode, String)> {
    let prefs = UserPreferencesRepository::upsert(&state.db, auth_user.user_id, body.sidebar_order)
        .await
        .map_err(db_error)?;

    Ok(Json(UserPreferencesResponse {
        sidebar_order: prefs.sidebar_order.0,
    }))
}
