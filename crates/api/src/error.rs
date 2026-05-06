use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ErrorDetail {
                code: code.into(),
                message: message.into(),
                details: None,
            },
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.error.details = Some(details);
        self
    }
}

pub struct AppError {
    pub status: StatusCode,
    pub body: ApiError,
}

impl AppError {
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            body: ApiError::new(code, message),
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.body = self.body.with_details(details);
        self
    }

    // Common errors
    // SECURITY: Use generic message to prevent resource enumeration attacks.
    // The specific resource type is only logged server-side.
    pub fn not_found(resource: &str) -> Self {
        tracing::debug!("Resource not found: {}", resource);
        Self::new(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "The requested resource was not found",
        )
    }

    pub fn forbidden(message: &str) -> Self {
        Self::new(StatusCode::FORBIDDEN, "FORBIDDEN", message)
    }

    pub fn conflict(code: &str, message: &str) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub fn bad_request(code: &str, message: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn internal(message: &str) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", message)
    }

    pub fn payment_required(code: &str, message: &str) -> Self {
        Self::new(StatusCode::PAYMENT_REQUIRED, code, message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("Database error: {}", e);
        Self::internal("Database error occurred")
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!("Internal error: {}", e);
        Self::internal("An internal error occurred")
    }
}

impl From<mcp_common::Error> for AppError {
    fn from(e: mcp_common::Error) -> Self {
        let status = match e.status_code() {
            401 => StatusCode::UNAUTHORIZED,
            403 => StatusCode::FORBIDDEN,
            404 => StatusCode::NOT_FOUND,
            409 => StatusCode::CONFLICT,
            400 => StatusCode::BAD_REQUEST,
            429 => StatusCode::TOO_MANY_REQUESTS,
            503 => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, e.error_code(), e.to_string())
    }
}

/// Helper function to convert internal errors to safe HTTP responses.
/// Logs the full error server-side but returns a sanitized message to the client.
///
/// # Usage
/// ```ignore
/// some_operation()
///     .await
///     .map_err(|e| internal_error("Database query failed", e))?;
/// ```
pub fn internal_error<E: std::fmt::Display>(context: &str, error: E) -> (StatusCode, String) {
    tracing::error!("{}: {}", context, error);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "An internal error occurred".to_string(),
    )
}

/// Helper for database errors - logs details but returns generic message
pub fn db_error<E: std::fmt::Display>(error: E) -> (StatusCode, String) {
    tracing::error!("Database error: {}", error);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "A database error occurred".to_string(),
    )
}

/// Helper for service unavailable errors
pub fn service_error<E: std::fmt::Display>(service: &str, error: E) -> (StatusCode, String) {
    tracing::error!("{} error: {}", service, error);
    (
        StatusCode::SERVICE_UNAVAILABLE,
        format!("{} is temporarily unavailable", service),
    )
}
