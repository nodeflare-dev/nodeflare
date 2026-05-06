use axum::http::HeaderMap;
use chrono::Datelike;
use fred::interfaces::{KeysInterface, LuaInterface};
use mcp_billing::Plan;
use mcp_db::{McpServer, WorkspaceRepository};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::{ProxyError, ProxyState};

/// Extract real client IP from request, handling reverse proxy headers
///
/// Security: Only trusts proxy headers when TRUST_PROXY_HEADERS=true
/// Priority: fly-client-ip > cf-connecting-ip > x-real-ip > x-forwarded-for > direct connection
pub fn extract_client_ip(headers: &HeaderMap, addr: &SocketAddr) -> String {
    let trust_proxy = std::env::var("TRUST_PROXY_HEADERS")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if !trust_proxy {
        return addr.ip().to_string();
    }

    // Fly.io specific header (most trusted when using Fly.io)
    if let Some(fly_ip) = headers.get("fly-client-ip").and_then(|v| v.to_str().ok()) {
        if is_valid_ip(fly_ip) {
            return fly_ip.to_string();
        }
    }

    // Cloudflare header
    if let Some(cf_ip) = headers.get("cf-connecting-ip").and_then(|v| v.to_str().ok()) {
        if is_valid_ip(cf_ip) {
            return cf_ip.to_string();
        }
    }

    // Nginx/generic reverse proxy header
    if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        if is_valid_ip(real_ip) {
            return real_ip.to_string();
        }
    }

    // X-Forwarded-For: take the first (leftmost) IP which is the original client
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first_ip) = xff.split(',').next().map(|s| s.trim()) {
            if is_valid_ip(first_ip) {
                return first_ip.to_string();
            }
        }
    }

    // Fall back to direct connection IP
    addr.ip().to_string()
}

/// Validate that a string looks like a valid IP address
fn is_valid_ip(ip: &str) -> bool {
    !ip.is_empty() && (ip.parse::<std::net::Ipv4Addr>().is_ok() || ip.parse::<std::net::Ipv6Addr>().is_ok())
}

const DEFAULT_RATE_LIMIT: i32 = 100; // requests per minute
const WINDOW_SIZE_SECONDS: i64 = 60;

// Brute force protection for API key validation
const API_KEY_BRUTE_FORCE_PREFIX: &str = "bf:apikey:";

/// Brute force protection configuration
struct BruteForceConfig {
    max_attempts: i64,
    lockout_secs: u64,
    attempt_window_secs: u64,
}

impl Default for BruteForceConfig {
    fn default() -> Self {
        Self {
            max_attempts: std::env::var("API_KEY_BRUTE_FORCE_MAX_ATTEMPTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            lockout_secs: std::env::var("API_KEY_BRUTE_FORCE_LOCKOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(600), // 10 minutes
            attempt_window_secs: std::env::var("API_KEY_BRUTE_FORCE_WINDOW_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300), // 5 minutes
        }
    }
}

/// Check if an IP is currently locked out due to API key brute force
pub async fn is_api_key_locked_out(state: &ProxyState, ip: &str) -> bool {
    let lockout_key = format!("{}lockout:{}", API_KEY_BRUTE_FORCE_PREFIX, ip);
    let exists: Result<bool, _> = state.redis.exists(&lockout_key).await;
    match &exists {
        Ok(_) => {}
        Err(e) => tracing::warn!("Redis error checking lockout for {}: {}", ip, e),
    }
    exists.unwrap_or(false)
}

/// Get remaining lockout time in seconds for API key brute force
pub async fn get_api_key_lockout_remaining(state: &ProxyState, ip: &str) -> Option<i64> {
    let lockout_key = format!("{}lockout:{}", API_KEY_BRUTE_FORCE_PREFIX, ip);
    let ttl: Result<i64, _> = state.redis.ttl(&lockout_key).await;
    ttl.ok().filter(|&t| t > 0)
}

/// Record a failed API key attempt and potentially lock out the IP
pub async fn record_api_key_failed_attempt(state: &ProxyState, ip: &str) {
    let config = BruteForceConfig::default();
    let attempts_key = format!("{}attempts:{}", API_KEY_BRUTE_FORCE_PREFIX, ip);
    let lockout_key = format!("{}lockout:{}", API_KEY_BRUTE_FORCE_PREFIX, ip);

    let lua_script = r#"
        local attempts = redis.call('INCR', KEYS[1])
        if attempts == 1 then
            redis.call('EXPIRE', KEYS[1], ARGV[1])
        end
        if attempts >= tonumber(ARGV[2]) then
            redis.call('SET', KEYS[2], '1', 'EX', ARGV[3])
            redis.call('DEL', KEYS[1])
        end
        return attempts
    "#;

    let result: Result<i64, _> = state
        .redis
        .eval(
            lua_script,
            &[attempts_key, lockout_key],
            &[
                config.attempt_window_secs.to_string(),
                config.max_attempts.to_string(),
                config.lockout_secs.to_string(),
            ],
        )
        .await;

    if let Ok(attempts) = result {
        if attempts >= config.max_attempts {
            tracing::warn!(
                "IP {} locked out after {} failed API key attempts (lockout: {}s)",
                ip,
                attempts,
                config.lockout_secs
            );
        }
    }
}

/// Clear failed API key attempts after successful validation
pub async fn clear_api_key_failed_attempts(state: &ProxyState, ip: &str) {
    let attempts_key = format!("{}attempts:{}", API_KEY_BRUTE_FORCE_PREFIX, ip);
    let _: Result<(), _> = state.redis.del(&attempts_key).await;
}

/// Lua script for atomic fixed window rate limiting
/// Uses a simple counter with expiration - more memory efficient than sliding window
/// Trade-off: slightly less accurate at window boundaries, but much better memory usage
/// Memory usage: O(1) per key instead of O(n) where n = requests per window
const RATE_LIMIT_SCRIPT: &str = r#"
local key = KEYS[1]
local limit = tonumber(ARGV[1])
local ttl = tonumber(ARGV[2])

-- Increment counter
local current = redis.call('INCR', key)

-- Set TTL only on first request (when counter is 1)
if current == 1 then
    redis.call('EXPIRE', key, ttl)
end

-- Check if limit exceeded
if current > limit then
    return -1
end

return current
"#;

pub async fn check(
    state: &ProxyState,
    credential_id: Uuid,
    server: &McpServer,
) -> Result<(), ProxyError> {
    // Use minute-based key for fixed window rate limiting
    let now = chrono::Utc::now();
    let minute_bucket = now.timestamp() / WINDOW_SIZE_SECONDS;
    let key = format!("rate_limit:{}:{}:{}", credential_id, server.id, minute_bucket);
    let limit = server.rate_limit_per_minute.unwrap_or(DEFAULT_RATE_LIMIT) as i64;
    // TTL should be slightly longer than window to handle edge cases
    let ttl = WINDOW_SIZE_SECONDS + 5;

    // Execute atomic rate limiting with Lua script
    let result: i64 = state
        .redis
        .eval(
            RATE_LIMIT_SCRIPT,
            &[key],
            &[limit.to_string(), ttl.to_string()],
        )
        .await
        .map_err(|e| ProxyError::Internal(format!("Redis error: {}", e)))?;

    if result < 0 {
        return Err(ProxyError::RateLimitExceeded);
    }

    Ok(())
}

/// Check monthly request quota based on workspace plan
pub async fn check_monthly_quota(
    state: &ProxyState,
    workspace_id: Uuid,
) -> Result<(), ProxyError> {
    // Get workspace to check plan
    let workspace = WorkspaceRepository::find_by_id(&state.db, workspace_id)
        .await
        .map_err(|e| ProxyError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ProxyError::Internal("Workspace not found".into()))?;

    // Check subscription status - block if past_due or cancelled
    if let Some(ref status) = workspace.subscription_status {
        if status == "past_due" || status == "unpaid" {
            return Err(ProxyError::PaymentRequired(
                "Your subscription payment is past due. Please update your payment method.".into()
            ));
        }
        if status == "cancelled" && workspace.plan != "free" {
            // If cancelled but not yet downgraded to free, check period end
            if let Some(period_end) = workspace.current_period_end {
                if chrono::Utc::now() > period_end {
                    return Err(ProxyError::PaymentRequired(
                        "Your subscription has expired. Please renew to continue.".into()
                    ));
                }
            }
        }
    }

    // Get plan limits
    let billing_plan = match workspace.plan.as_str() {
        "pro" => Plan::Pro,
        "team" => Plan::Team,
        "enterprise" => Plan::Enterprise,
        _ => Plan::Free,
    };
    let limits = billing_plan.limits();

    // Scalability: All plans now have finite limits (Enterprise = 1B/month)
    // This prevents resource exhaustion while still providing very high limits

    // Get current month key
    let now = chrono::Utc::now();
    let month_key = format!(
        "monthly_requests:{}:{:04}-{:02}",
        workspace_id,
        now.year(),
        now.month()
    );

    // Get current count from Redis
    let count: Option<i64> = state
        .redis
        .get(&month_key)
        .await
        .map_err(|e| ProxyError::Internal(format!("Redis error: {}", e)))?;

    let current_count = count.unwrap_or(0) as u64;

    if current_count >= limits.max_requests_per_month {
        return Err(ProxyError::QuotaExceeded(format!(
            "Monthly request quota exceeded ({}/{}). Please upgrade your plan.",
            current_count, limits.max_requests_per_month
        )));
    }

    Ok(())
}

/// Increment monthly request counter
pub async fn increment_monthly_counter(
    state: &ProxyState,
    workspace_id: Uuid,
) -> Result<(), ProxyError> {
    let now = chrono::Utc::now();
    let month_key = format!(
        "monthly_requests:{}:{:04}-{:02}",
        workspace_id,
        now.year(),
        now.month()
    );

    // Increment counter
    let _: i64 = state
        .redis
        .incr(&month_key)
        .await
        .map_err(|e| ProxyError::Internal(format!("Redis error: {}", e)))?;

    // Set TTL to expire after this month (add some buffer days)
    // Calculate days remaining in month + 5 days buffer
    let days_in_month = match now.month() {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if now.year() % 4 == 0 && (now.year() % 100 != 0 || now.year() % 400 == 0) { 29 } else { 28 },
        _ => 31,
    };
    let ttl_seconds = ((days_in_month - now.day() + 5) as i64) * 24 * 60 * 60;

    let _: () = state
        .redis
        .expire(&month_key, ttl_seconds)
        .await
        .map_err(|e| ProxyError::Internal(format!("Redis error: {}", e)))?;

    Ok(())
}
