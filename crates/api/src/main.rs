use anyhow::Result;
use axum::{
    http::{header, HeaderName, HeaderValue, Method},
    middleware as axum_middleware,
    routing::get,
    Router,
};
use fred::interfaces::ClientLike;
use mcp_common::AppConfig;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cache;
mod error;
mod extractors;
mod middleware;
mod redis_subscriber;
mod routes;
mod state;
mod ws_manager;

use middleware::rate_limit_middleware;
use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // Install rustls crypto provider (required for TLS connections)
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mcp_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = AppConfig::from_env()?;
    tracing::info!("Starting MCP Cloud API server");

    // Create database pool
    let db_pool = mcp_db::create_pool(&config).await?;

    // Create Redis client with reconnection policy
    let redis_config = fred::types::RedisConfig::from_url(&config.redis.url)?;
    let reconnect_policy = fred::types::ReconnectPolicy::new_exponential(0, 1, 30_000, 2);
    let redis = fred::prelude::RedisClient::new(redis_config, None, None, Some(reconnect_policy));
    redis.connect();
    redis.wait_for_connect().await?;
    tracing::info!("Connected to Redis");

    // Create job queue for background tasks
    let job_queue = Arc::new(mcp_queue::JobQueue::connect(&config.redis.url).await?);
    tracing::info!("Connected to job queue");

    // Start job queue keep-alive task to prevent Upstash idle timeout
    job_queue.clone().start_keepalive_task();
    tracing::info!("Job queue keep-alive task started");

    // Create GitHub App client (optional)
    let github = mcp_github::GitHubApp::new(&config).ok();
    if github.is_some() {
        tracing::info!("GitHub App initialized");
    } else {
        tracing::warn!("GitHub App not configured - private repos will not be accessible");
    }

    // Create app state
    let state = Arc::new(AppState::new(config.clone(), db_pool.clone(), redis, job_queue, github));

    // Start WsManager cleanup task
    let ws_manager_arc = Arc::new(state.ws_manager.clone());
    ws_manager_arc.clone().start_cleanup_task();
    tracing::info!("WsManager cleanup task started");

    // Start Redis subscriber for WebSocket events
    redis_subscriber::start_redis_subscriber(
        &config.redis.url,
        ws_manager_arc,
    )
    .await;
    tracing::info!("Redis subscriber started for WebSocket events");

    // Start request_logs cleanup task
    start_request_logs_cleanup_task(db_pool.clone());
    tracing::info!("Request logs cleanup task started");

    // Start deployment timeout task
    start_deployment_timeout_task(db_pool);
    tracing::info!("Deployment timeout task started");

    // Build router
    let app = create_router(state);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("API server listening on {}", addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

/// Start background task to clean up old request_logs
fn start_request_logs_cleanup_task(db_pool: mcp_db::DbPool) {
    use chrono::{Duration, Utc};
    use mcp_db::RequestLogRepository;

    // Get retention days from env (default: 30 days)
    let retention_days: i64 = std::env::var("REQUEST_LOGS_RETENTION_DAYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    // Get cleanup interval from env (default: 1 hour)
    let cleanup_interval_secs: u64 = std::env::var("REQUEST_LOGS_CLEANUP_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3600);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(cleanup_interval_secs));
        loop {
            interval.tick().await;
            let cutoff = Utc::now() - Duration::days(retention_days);
            match RequestLogRepository::delete_old_logs(&db_pool, cutoff).await {
                Ok(deleted) => {
                    if deleted > 0 {
                        tracing::info!("Cleaned up {} old request logs (older than {} days)", deleted, retention_days);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to clean up old request logs: {}", e);
                }
            }
        }
    });
}

/// Start background task to timeout stuck deployments
fn start_deployment_timeout_task(db_pool: mcp_db::DbPool) {
    use mcp_db::DeploymentRepository;

    // Get timeout in minutes from env (default: 15 minutes)
    let timeout_minutes: i64 = std::env::var("DEPLOYMENT_TIMEOUT_MINUTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(15);

    // Check every minute
    let check_interval_secs: u64 = std::env::var("DEPLOYMENT_TIMEOUT_CHECK_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(check_interval_secs));
        loop {
            interval.tick().await;
            match DeploymentRepository::timeout_stuck_deployments(&db_pool, timeout_minutes).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!("Timed out {} stuck deployments (older than {} minutes)", count, timeout_minutes);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to timeout stuck deployments: {}", e);
                }
            }
        }
    });
}

fn create_router(state: Arc<AppState>) -> Router {
    // CORS configuration for frontend
    let frontend_url = state.config.server.frontend_url.clone();
    let cors = CorsLayer::new()
        .allow_origin(
            frontend_url
                .parse::<HeaderValue>()
                .unwrap_or_else(|_| HeaderValue::from_static("http://localhost:3000")),
        )
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::ORIGIN,
            HeaderName::from_static("x-requested-with"),
        ])
        .allow_credentials(true)
        .max_age(std::time::Duration::from_secs(3600));

    // SECURITY NOTE: OAuth endpoints use permissive CORS intentionally.
    // This is required by OAuth 2.0 spec since clients can be on any domain.
    // Security is enforced at the application level through:
    // - Client authentication (client_id/secret) for confidential clients
    // - PKCE (Proof Key for Code Exchange) for public clients
    // - Rate limiting on dynamic client registration (10/hour per IP)
    // - Redirect URI validation to prevent authorization code theft
    // Do NOT use allow_credentials(true) with Any origin.
    let oauth_cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
        ])
        .max_age(std::time::Duration::from_secs(3600));

    // Check if rate limiting is enabled (default: true in production)
    let rate_limit_enabled = std::env::var("RATE_LIMIT_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true);

    // Get request body limit from env (default: 1MB)
    let body_limit: usize = std::env::var("REQUEST_BODY_LIMIT_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024 * 1024); // 1MB default

    // Main router with frontend CORS
    let main_router = Router::new()
        // Health check (no rate limiting)
        .route("/health", get(routes::health::health_check))
        .route("/ready", get(routes::health::readiness_check))
        // API v1 with rate limiting
        .nest("/api/v1", routes::api_router())
        // WebSocket endpoints (rate limiting handled at connection level)
        .nest("/ws", routes::ws_router())
        // OpenAPI docs
        .merge(routes::openapi::openapi_router())
        .layer(cors);

    // OAuth routes with permissive CORS (external clients like Claude need to access these)
    let oauth_router = Router::new()
        .route("/.well-known/oauth-authorization-server", get(routes::oauth::authorization_server_metadata))
        .route("/oauth/authorize", get(routes::oauth::authorize))
        .route("/oauth/token", axum::routing::post(routes::oauth::token))
        .route("/oauth/register", axum::routing::post(routes::oauth::register_client))
        .layer(oauth_cors);

    // Merge routers
    let router = Router::new()
        .merge(main_router)
        .merge(oauth_router);

    // Apply rate limiting middleware conditionally
    let router = if rate_limit_enabled {
        router.layer(axum_middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
    } else {
        router
    };

    router
        // Middleware (applied to all routes)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(body_limit))
        // Security headers
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
        ))
        // HSTS - Enforce HTTPS for 1 year, including subdomains
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
        ))
        // Content-Security-Policy - Restrict resource loading
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; \
                 script-src 'self' 'unsafe-inline' 'unsafe-eval'; \
                 style-src 'self' 'unsafe-inline'; \
                 img-src 'self' data: https:; \
                 font-src 'self' data:; \
                 connect-src 'self' wss: https:; \
                 frame-ancestors 'none'; \
                 base-uri 'self'; \
                 form-action 'self'"
            ),
        ))
        .with_state(state)
}
