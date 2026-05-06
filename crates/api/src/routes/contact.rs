use axum::{
    extract::{ConnectInfo, State},
    Json,
};
use mcp_db::{models::CreateContactMessage, repositories::ContactMessageRepository};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::{Arc, LazyLock}};

// Static regex for email validation - compiled once, no unwrap panic risk at runtime
static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$"
    ).expect("Invalid email regex pattern - this is a compile-time constant")
});

// Static spam patterns - compiled once
static SPAM_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)buy\s+now",
        r"(?i)click\s+here",
        r"(?i)free\s+money",
        r"(?i)winner",
        r"(?i)congratulations.*won",
        r"(?i)casino",
        r"(?i)viagra",
        r"(?i)cryptocurrency.*invest",
        r"(?i)bitcoin.*profit",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

use crate::{error::AppError, state::AppState};

// Constants for validation
const MAX_NAME_LENGTH: usize = 100;
const MAX_EMAIL_LENGTH: usize = 254;
const MAX_MESSAGE_LENGTH: usize = 5000;
const MIN_MESSAGE_LENGTH: usize = 10;
const RATE_LIMIT_KEY_PREFIX: &str = "contact_rate:";
const RATE_LIMIT_WINDOW_SECS: u64 = 3600; // 1 hour
const RATE_LIMIT_MAX_REQUESTS: i64 = 5; // 5 requests per hour per IP

#[derive(Debug, Deserialize)]
pub struct ContactRequest {
    pub name: String,
    pub email: String,
    pub message: String,
    #[serde(default)]
    pub honeypot: Option<String>, // Hidden field for bot detection
}

#[derive(Debug, Serialize)]
pub struct ContactResponse {
    pub success: bool,
    pub message: String,
}

/// Sanitize input by removing potentially dangerous characters
fn sanitize_input(input: &str) -> String {
    input
        .trim()
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect::<String>()
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Validate email format with strict regex
fn is_valid_email(email: &str) -> bool {
    EMAIL_REGEX.is_match(email) && email.len() <= MAX_EMAIL_LENGTH
}

/// Check for spam patterns in message
fn contains_spam_patterns(message: &str) -> bool {
    SPAM_PATTERNS.iter().any(|re| re.is_match(message))
}

/// Check rate limit using Redis with atomic Lua script (fixes race condition)
async fn check_rate_limit(state: &AppState, ip: &str) -> Result<bool, AppError> {
    let key = format!("{}{}", RATE_LIMIT_KEY_PREFIX, ip);

    // Use Lua script for atomic INCR + EXPIRE operation
    let lua_script = r#"
        local current = redis.call('INCR', KEYS[1])
        if current == 1 then
            redis.call('EXPIRE', KEYS[1], ARGV[1])
        end
        return current
    "#;

    let count: i64 = fred::interfaces::LuaInterface::eval(
        &state.redis,
        lua_script,
        vec![key],
        vec![RATE_LIMIT_WINDOW_SECS.to_string()],
    )
    .await
    .map_err(|_| AppError::internal("Rate limit check failed"))?;

    Ok(count <= RATE_LIMIT_MAX_REQUESTS)
}

pub async fn submit_contact(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<ContactRequest>,
) -> Result<Json<ContactResponse>, AppError> {
    // Honeypot check - if filled, it's likely a bot
    if let Some(ref honeypot) = payload.honeypot {
        if !honeypot.is_empty() {
            tracing::warn!("Honeypot triggered from IP: {}", addr.ip());
            // Return success to not reveal detection
            return Ok(Json(ContactResponse {
                success: true,
                message: "Thank you for your message.".to_string(),
            }));
        }
    }

    // Rate limiting
    let ip = addr.ip().to_string();
    if !check_rate_limit(&state, &ip).await? {
        tracing::warn!("Rate limit exceeded for IP: {}", ip);
        return Err(AppError::new(
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "Too many requests. Please try again later.",
        ));
    }

    // Sanitize inputs
    let name = sanitize_input(&payload.name);
    let email = payload.email.trim().to_lowercase();
    let message = sanitize_input(&payload.message);

    // Validate name
    if name.is_empty() {
        return Err(AppError::bad_request("INVALID_NAME", "Name is required"));
    }
    if name.len() > MAX_NAME_LENGTH {
        return Err(AppError::bad_request(
            "NAME_TOO_LONG",
            &format!("Name must be {} characters or less", MAX_NAME_LENGTH),
        ));
    }

    // Validate email
    if !is_valid_email(&email) {
        return Err(AppError::bad_request(
            "INVALID_EMAIL",
            "Please enter a valid email address",
        ));
    }

    // Validate message
    if message.is_empty() {
        return Err(AppError::bad_request("INVALID_MESSAGE", "Message is required"));
    }
    if message.len() < MIN_MESSAGE_LENGTH {
        return Err(AppError::bad_request(
            "MESSAGE_TOO_SHORT",
            &format!("Message must be at least {} characters", MIN_MESSAGE_LENGTH),
        ));
    }
    if message.len() > MAX_MESSAGE_LENGTH {
        return Err(AppError::bad_request(
            "MESSAGE_TOO_LONG",
            &format!("Message must be {} characters or less", MAX_MESSAGE_LENGTH),
        ));
    }

    // Spam detection
    if contains_spam_patterns(&message) {
        tracing::warn!("Spam pattern detected from IP: {}", ip);
        // Return success to not reveal detection
        return Ok(Json(ContactResponse {
            success: true,
            message: "Thank you for your message.".to_string(),
        }));
    }

    // Save to database
    ContactMessageRepository::create(
        &state.db,
        CreateContactMessage {
            name: name.clone(),
            email: email.clone(),
            message: message.clone(),
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to save contact message: {}", e);
        AppError::internal("Failed to save message")
    })?;

    // Send email notification to admin (non-blocking)
    if let Some(ref email_service) = state.email {
        let admin_email = std::env::var("ADMIN_EMAIL").unwrap_or_default();
        if !admin_email.is_empty() {
            let email_service = email_service.clone();
            let sender_name = name.clone();
            let sender_email = email.clone();
            let msg = message.clone();
            tokio::spawn(async move {
                if let Err(e) = email_service
                    .send_contact_notification(&admin_email, &sender_name, &sender_email, &msg)
                    .await
                {
                    tracing::error!("Failed to send contact notification email: {}", e);
                }
            });
        }
    }

    tracing::info!("Contact message received from IP: {}", ip);

    Ok(Json(ContactResponse {
        success: true,
        message: "Thank you for your message. We will get back to you soon.".to_string(),
    }))
}
