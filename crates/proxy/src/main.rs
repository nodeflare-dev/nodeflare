use anyhow::Result;
use axum::{
    body::Body,
    extract::{Host, State},
    http::{header::HeaderValue, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{any, get},
    Json, Router,
};
use bytes::Bytes;
use fred::interfaces::ClientLike;
use futures::StreamExt;
use mcp_common::{AppConfig, McpMethod};
use mcp_db::ServerRepository;
use auth::AuthCredential;
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;
use tokio::net::TcpListener;
use tower_http::{limit::RequestBodyLimitLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod affinity;
mod auth;
mod cache;
mod embedding;
mod mcp_transform;
mod meta_tools;
mod rate_limit;
mod redis_cache;

use cache::{RequestCache, CoalesceResult};
use mcp_db::{Tool, ToolRepository, UpsertTool};
use redis_cache::{RedisCache, CachedServer};

pub struct ProxyState {
    pub config: AppConfig,
    pub db: mcp_db::DbPool,
    pub redis: fred::prelude::RedisClient,
    pub http_client: reqwest::Client,
    /// Long-lived client for SSE/streaming upstream requests (idle timeout, no total
    /// timeout). Built once and reused so connections are pooled across requests.
    pub sse_client: reqwest::Client,
    pub request_cache: RequestCache,
    pub redis_cache: RedisCache,
    /// Maximum request body size accepted from clients / read from upstreams.
    pub body_limit: usize,
    /// Gemini embedding client for semantic search_tools. `None` disables semantic
    /// search (falls back to lexical) when GEMINI_API_KEY is unset.
    pub embedding: Option<embedding::EmbeddingClient>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mcp_proxy=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env()?;
    tracing::info!("Starting MCP Cloud Proxy Gateway");

    let db_pool = mcp_db::create_pool(&config).await?;

    let redis_config = fred::types::RedisConfig::from_url(&config.redis.url)?;

    // Configure Redis performance with short timeouts to prevent blocking Claude connections
    let perf_config = fred::types::PerformanceConfig {
        default_command_timeout: std::time::Duration::from_secs(2),
        ..Default::default()
    };

    // Configure connection with short timeouts - Redis failures should not block proxy
    let conn_config = fred::types::ConnectionConfig {
        connection_timeout: std::time::Duration::from_secs(2),
        internal_command_timeout: std::time::Duration::from_secs(2),
        ..Default::default()
    };

    // Reconnection policy: exponential backoff, retry forever (max_attempts=0)
    // This handles Upstash closing idle connections
    let reconnect_policy = fred::types::ReconnectPolicy::new_exponential(
        0,     // max_attempts: 0 = retry forever
        100,   // min_delay_ms: start with 100ms
        5000,  // max_delay_ms: cap at 5 seconds
        2,     // multiplier: double delay each attempt
    );

    let redis = fred::prelude::RedisClient::new(
        redis_config,
        Some(perf_config),
        Some(conn_config),
        Some(reconnect_policy),
    );
    redis.connect();
    tracing::info!("Connecting to Redis...");
    redis.wait_for_connect().await?;
    tracing::info!("Redis connected successfully with auto-reconnect enabled");

    // Upstream timeout is configurable; long-running tool calls were being cut at the
    // old hard-coded 30s. Also set a pool idle timeout so we don't pin dead connections.
    let upstream_timeout_secs: u64 = std::env::var("PROXY_UPSTREAM_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let pool_idle_timeout_secs: u64 = std::env::var("PROXY_POOL_IDLE_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(upstream_timeout_secs))
        .pool_idle_timeout(std::time::Duration::from_secs(pool_idle_timeout_secs))
        .build()?;

    // SSE/streaming client: built once and reused (was previously rebuilt per
    // request, defeating connection pooling). No total timeout — MCP SSE streams are
    // legitimately long-lived — but a read (idle) timeout drops silently hung upstreams.
    let sse_idle_timeout_secs: u64 = std::env::var("PROXY_SSE_IDLE_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);
    let sse_client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .read_timeout(std::time::Duration::from_secs(sse_idle_timeout_secs))
        .pool_idle_timeout(std::time::Duration::from_secs(pool_idle_timeout_secs))
        .build()?;

    // Request cache: TTL and max entries from environment
    let cache_ttl: u64 = std::env::var("PROXY_CACHE_TTL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let cache_max_entries: usize = std::env::var("PROXY_CACHE_MAX_ENTRIES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10000);
    let request_cache = RequestCache::new(cache_ttl, cache_max_entries);

    // Redis cache for API keys and server metadata
    let redis_cache = RedisCache::new(redis.clone());

    // Get request body limit from env (default: 10MB for proxy)
    let body_limit: usize = std::env::var("PROXY_BODY_LIMIT_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10 * 1024 * 1024); // 10MB default for proxy

    // Optional Gemini embedding client for semantic search_tools (None = lexical only).
    let embedding = embedding::EmbeddingClient::from_env(http_client.clone());

    let state = Arc::new(ProxyState {
        config: config.clone(),
        db: db_pool,
        redis,
        http_client,
        sse_client,
        request_cache,
        redis_cache,
        body_limit,
        embedding,
    });

    let app = Router::new()
        .route("/health", any(health_check))
        // OAuth 2.1 metadata endpoint (RFC 8414)
        .route("/.well-known/oauth-authorization-server", get(oauth_metadata))
        // OAuth 2.0 Protected Resource Metadata (RFC 9728)
        .route("/.well-known/oauth-protected-resource", get(protected_resource_metadata))
        // Subdomain-based routing: {slug}.mcp.cloud/* -> MCP server
        .fallback(any(proxy_handler))
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(body_limit))
        .with_state(state);

    let addr = format!("{}:{}", config.server.host, config.server.proxy_port);
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("Proxy gateway listening on {}", addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "ok"
}

/// OAuth 2.1 Authorization Server Metadata (RFC 8414)
#[derive(Debug, Serialize)]
struct OAuthServerMetadata {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: String,
    response_types_supported: Vec<String>,
    grant_types_supported: Vec<String>,
    code_challenge_methods_supported: Vec<String>,
    token_endpoint_auth_methods_supported: Vec<String>,
}

/// OAuth metadata endpoint - returns API server's OAuth endpoints
async fn oauth_metadata(
    State(state): State<Arc<ProxyState>>,
    Host(host): Host,
) -> Response {
    tracing::info!("OAuth metadata requested from proxy, host={}", host);

    // If the target server has NodeFlare auth disabled, do NOT advertise OAuth
    // authorization-server metadata. Some MCP clients key auth discovery off this
    // endpoint (the older 2025-03-26 flow) and would start an OAuth flow even though
    // the proxy forwards requests without requiring any credentials. Returning 404
    // keeps the "no auth" contract consistent with the protected-resource and MCP
    // endpoints (both already signal "not protected" for auth-disabled servers).
    // Note: if the subdomain can't be resolved to a running server we fall through to
    // advertising metadata to avoid changing edge-case responses.
    if let Ok(slug) = extract_subdomain(&host, &state.config.server.proxy_base_domain) {
        if let Ok(server) = resolve_server(&state, &slug).await {
            if !server.auth_enabled {
                tracing::info!(
                    "Auth disabled for server {}, not advertising OAuth authorization-server metadata",
                    slug
                );
                return StatusCode::NOT_FOUND.into_response();
            }
        }
    }

    let api_url = std::env::var("API_URL").unwrap_or_else(|_| {
        format!("http://{}:{}", state.config.server.host, state.config.server.port)
    });

    tracing::info!("OAuth metadata: api_url={}", api_url);

    Json(OAuthServerMetadata {
        issuer: api_url.clone(),
        authorization_endpoint: format!("{}/oauth/authorize", api_url),
        token_endpoint: format!("{}/oauth/token", api_url),
        registration_endpoint: format!("{}/oauth/register", api_url),
        response_types_supported: vec!["code".to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        code_challenge_methods_supported: vec!["S256".to_string()],
        token_endpoint_auth_methods_supported: vec![
            "client_secret_basic".to_string(),
            "client_secret_post".to_string(),
            "none".to_string(),
        ],
    })
    .into_response()
}

/// OAuth 2.0 Protected Resource Metadata (RFC 9728)
#[derive(Debug, Serialize)]
struct ProtectedResourceMetadata {
    resource: String,
    authorization_servers: Vec<String>,
    scopes_supported: Vec<String>,
}

/// OAuth 2.0 Protected Resource Metadata endpoint (RFC 9728)
async fn protected_resource_metadata(
    State(state): State<Arc<ProxyState>>,
    Host(host): Host,
) -> Response {
    tracing::info!("Protected resource metadata requested from proxy, host={}", host);

    // If the target server has NodeFlare auth disabled, do NOT advertise OAuth
    // protected-resource metadata. Otherwise MCP clients (which probe this endpoint
    // during discovery per RFC 9728) would enforce auth client-side even though the
    // proxy forwards requests without requiring any credentials.
    // Note: if the subdomain can't be resolved to a running server we fall through to
    // the previous behavior (advertise metadata) to avoid changing edge-case responses.
    if let Ok(slug) = extract_subdomain(&host, &state.config.server.proxy_base_domain) {
        if let Ok(server) = resolve_server(&state, &slug).await {
            if !server.auth_enabled {
                tracing::info!(
                    "Auth disabled for server {}, not advertising OAuth protected-resource metadata",
                    slug
                );
                return StatusCode::NOT_FOUND.into_response();
            }
        }
    }

    let api_url = std::env::var("API_URL").unwrap_or_else(|_| {
        format!("http://{}:{}", state.config.server.host, state.config.server.port)
    });

    // Resource is this proxy endpoint (RFC 9728 format)
    let resource = format!("https://{}", host);
    tracing::info!("Protected resource metadata: resource={}, authorization_servers=[{}]", resource, api_url);

    Json(ProtectedResourceMetadata {
        resource,
        authorization_servers: vec![api_url],
        scopes_supported: vec![
            "*".to_string(),
            "tools:*".to_string(),
            "tools:list".to_string(),
            "tools:call".to_string(),
            "resources:*".to_string(),
            "resources:list".to_string(),
            "resources:read".to_string(),
            "prompts:*".to_string(),
            "prompts:list".to_string(),
            "prompts:get".to_string(),
        ],
    })
    .into_response()
}

async fn proxy_handler(
    State(state): State<Arc<ProxyState>>,
    Host(host): Host,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    uri: Uri,
    request: Request<Body>,
) -> Result<Response, ProxyError> {
    let start = Instant::now();

    // Detect browser access and redirect to main site
    // Browsers send Accept: text/html, while MCP clients send Accept: application/json or text/event-stream
    // Only treat it as a browser navigation when it asks for HTML and does NOT also
    // accept an MCP content type. MCP clients send Accept: application/json and/or
    // text/event-stream, so requiring their absence avoids redirecting a real MCP
    // client that happens to include text/html in a broad Accept header.
    let is_browser = request.method() == axum::http::Method::GET
        && request
            .headers()
            .get(axum::http::header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .map(|accept| {
                accept.contains("text/html")
                    && !accept.contains("application/json")
                    && !accept.contains("text/event-stream")
            })
            .unwrap_or(false);

    if is_browser {
        tracing::info!("Browser access detected, redirecting to main site");
        let main_site_url = std::env::var("MAIN_SITE_URL").unwrap_or_else(|_| "https://nodeflare.tech".to_string());
        return Ok(Response::builder()
            .status(StatusCode::FOUND)
            .header(axum::http::header::LOCATION, main_site_url)
            .body(Body::empty())
            .unwrap());
    }

    // Extract real client IP (handles reverse proxy headers when TRUST_PROXY_HEADERS=true)
    let client_ip = rate_limit::extract_client_ip(request.headers(), &addr);

    tracing::info!("proxy_handler: host={}, uri={}, client_ip={}, base_domain={}",
        host, uri, client_ip, state.config.server.proxy_base_domain);

    // 1. Extract server slug from subdomain
    // e.g., "my-server.mcp.cloud" -> "my-server"
    let server_slug = extract_subdomain(&host, &state.config.server.proxy_base_domain)?;
    tracing::info!("proxy_handler: extracted slug={}", server_slug);

    // Helper to add host to Unauthorized errors for proper WWW-Authenticate header
    let host_str = host.clone();
    let add_host_to_error = |e: ProxyError| -> ProxyError {
        match e {
            ProxyError::Unauthorized(msg, _) => ProxyError::Unauthorized(msg, Some(host_str.clone())),
            other => other,
        }
    };

    // 2. Resolve server first to check auth_enabled
    let server = resolve_server(&state, &server_slug).await?;
    tracing::info!("proxy_handler: server resolved, id={}, auth_enabled={}", server.id, server.auth_enabled);

    // 2b. Status gate: only serve servers that are actually running. Without this a
    // stopped/building/failed/deleting server would keep accepting traffic (and pay
    // the full upstream timeout failing) instead of returning promptly.
    if !server.is_serveable() {
        tracing::warn!("proxy_handler: server {} not serveable (status={})", server.id, server.status);
        return Err(ProxyError::ServiceUnavailable(format!(
            "Server is not running (status: {})",
            server.status
        )));
    }

    // 3. Handle auth based on server.auth_enabled setting
    let credential: Option<AuthCredential> = if server.auth_enabled {
        // Auth enabled: Extract and validate API key
        let api_key = auth::extract_api_key(&request).map_err(|e| add_host_to_error(e))?;
        tracing::debug!("proxy_handler: API key extracted (prefix={}...)", &api_key[..api_key.len().min(8)]);

        let credential = auth::validate_credential(&state, &api_key, &client_ip)
            .await
            .map_err(|e| add_host_to_error(e))?;
        tracing::info!("proxy_handler: credential validated successfully (took {:?})", start.elapsed());

        // Verify credential has access to this server
        if let Some(cred_server_id) = credential.server_id() {
            tracing::debug!("proxy_handler: checking server access, cred_server_id={}, server.id={}", cred_server_id, server.id);
            if cred_server_id != server.id {
                tracing::warn!("proxy_handler: credential not valid for this server");
                return Err(ProxyError::Forbidden("Credential not valid for this server".into()));
            }
        }

        // Check rate limit (per-minute) - graceful degradation on Redis errors
        tracing::debug!("proxy_handler: checking rate limit");
        match rate_limit::check(&state, credential.id(), &server).await {
            Ok(_) => tracing::debug!("proxy_handler: rate limit check passed"),
            Err(ProxyError::RateLimitExceeded) => return Err(ProxyError::RateLimitExceeded),
            Err(e) => tracing::warn!("proxy_handler: rate limit check failed (continuing anyway): {}", e),
        }

        Some(credential)
    } else {
        // Auth disabled: Skip credential validation
        tracing::info!("proxy_handler: auth disabled for server {}, skipping credential validation", server.id);
        None
    };

    // 4. Atomically check + increment the monthly quota before forwarding.
    // (This applies regardless of auth_enabled to prevent abuse.) Incrementing
    // inline avoids the previous check-then-fire-and-forget TOCTOU; on a hard
    // (non-quota) error we fail open but log.
    tracing::debug!("proxy_handler: checking monthly quota");
    match rate_limit::check_and_increment_monthly_quota(&state, server.workspace_id).await {
        Ok(_) => tracing::debug!("proxy_handler: monthly quota check passed"),
        Err(e @ ProxyError::QuotaExceeded(_)) => return Err(e),
        Err(e @ ProxyError::PaymentRequired(_)) => return Err(e),
        Err(e @ ProxyError::RateLimitExceeded) => return Err(e),
        Err(e) => tracing::warn!("proxy_handler: monthly quota check failed (continuing anyway): {}", e),
    }

    // 5. Forward request to MCP server
    let endpoint_url = server
        .endpoint_url
        .as_ref()
        .ok_or_else(|| {
            tracing::error!("proxy_handler: server {} has no endpoint_url (not deployed)", server.id);
            ProxyError::ServiceUnavailable("Server not deployed".into())
        })?;

    // Use server's mcp_path (e.g., "/mcp", "/api", "/sse") - configurable per server
    let mcp_path = server.mcp_path.trim_start_matches('/');
    let query = uri.query().map(|q| format!("?{}", q)).unwrap_or_default();
    let target_url = format!("{}/{}{}", endpoint_url.trim_end_matches('/'), mcp_path, query);
    tracing::info!("proxy_handler: forwarding request to {}", target_url);

    // 6. Forward request. Scope checks only apply when NodeFlare auth is enabled
    // (credential present); otherwise the upstream handles its own auth.
    let fwd = ForwardContext {
        auth_enabled: server.auth_enabled,
        client_ip: &client_ip,
        host: &host,
        server_id: server.id,
        filter_by_scope: server.tool_list_filter_by_scope,
        slim: server.tool_schema_slim,
        search_mode: server.tool_search_mode,
    };
    let (response, mcp_info) =
        forward_request(&state, &target_url, request, credential.as_ref(), &fwd).await?;
    tracing::info!("proxy_handler: request forwarded, status={}", response.status());

    // 7. Monthly quota is already incremented atomically in step 4 (before forwarding).

    // 8. Log request (async, don't block)
    let duration_ms = start.elapsed().as_millis() as i32;
    let server_id = server.id;
    let api_key_id = credential.as_ref().map(|c| c.id());
    let status = if response.status().is_success() {
        "success"
    } else {
        "error"
    };
    let tool_name = mcp_info.target.clone();

    let db = state.db.clone();
    tokio::spawn(async move {
        let _ = mcp_db::RequestLogRepository::create(
            &db,
            mcp_db::CreateRequestLog {
                server_id,
                tool_name,
                api_key_id,
                client_info: None,
                request_body: None,
                response_status: status.to_string(),
                error_message: None,
                duration_ms,
            },
        )
        .await;
    });

    Ok(response)
}

async fn resolve_server(state: &ProxyState, slug: &str) -> Result<CachedServer, ProxyError> {
    tracing::debug!("resolve_server: looking up slug={}", slug);

    // Try Redis cache first
    let cache_start = Instant::now();
    if let Some(cached) = state.redis_cache.get_server(slug).await {
        tracing::debug!("resolve_server: cache HIT for slug={} (took {:?})", slug, cache_start.elapsed());
        return Ok(cached);
    }
    tracing::debug!("resolve_server: cache MISS for slug={} (took {:?})", slug, cache_start.elapsed());

    // Cache miss - query database
    let db_start = Instant::now();
    tracing::debug!("resolve_server: querying database for slug={}", slug);
    let server = ServerRepository::find_by_endpoint_slug(&state.db, slug)
        .await
        .map_err(|e| {
            tracing::error!("resolve_server: database error for slug={} (took {:?}): {}", slug, db_start.elapsed(), e);
            ProxyError::Internal(e.to_string())
        })?
        .ok_or_else(|| {
            tracing::warn!("resolve_server: server NOT found for slug={} (took {:?})", slug, db_start.elapsed());
            ProxyError::NotFound("Server not found".into())
        })?;

    tracing::info!("resolve_server: found server id={} for slug={} (took {:?})", server.id, slug, db_start.elapsed());

    // Cache the result (async, don't block)
    let redis_cache = state.redis_cache.clone();
    let server_clone = server.clone();
    let slug_owned = slug.to_string();
    tokio::spawn(async move {
        redis_cache.set_server(&slug_owned, &server_clone).await;
    });

    Ok(CachedServer::from(&server))
}

/// Extracted MCP request info for scope checking and logging
#[derive(Debug, Clone)]
struct McpRequestInfo {
    method: McpMethod,
    method_str: Option<String>,
    target: Option<String>,
    /// The JSON-RPC request `id`, echoed back in error responses for correlation.
    id: Option<serde_json::Value>,
}

/// Extract MCP method and target from JSON-RPC request body
fn extract_mcp_request_info(body: &[u8]) -> McpRequestInfo {
    let mut info = McpRequestInfo {
        method: McpMethod::Unknown,
        method_str: None,
        target: None,
        id: None,
    };

    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
        info.id = json.get("id").cloned();
        if let Some(method_str) = json.get("method").and_then(|m| m.as_str()) {
            info.method_str = Some(method_str.to_string());
            info.method = McpMethod::parse(method_str);

            // Extract target based on method type
            match info.method {
                McpMethod::ToolsCall => {
                    // Extract tool name from params.name
                    info.target = json
                        .get("params")
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .map(String::from);
                }
                McpMethod::ResourcesRead => {
                    // Extract resource URI from params.uri
                    info.target = json
                        .get("params")
                        .and_then(|p| p.get("uri"))
                        .and_then(|u| u.as_str())
                        .map(String::from);
                }
                McpMethod::PromptsGet => {
                    // Extract prompt name from params.name
                    info.target = json
                        .get("params")
                        .and_then(|p| p.get("name"))
                        .and_then(|n| n.as_str())
                        .map(String::from);
                }
                _ => {}
            }
        }
    }

    info
}

/// Check if credential has permission for the MCP request
fn check_scope_permission(credential: &AuthCredential, mcp_info: &McpRequestInfo) -> Result<(), ProxyError> {
    // Unknown methods are allowed (forward compatibility)
    if matches!(mcp_info.method, McpMethod::Unknown) {
        return Ok(());
    }

    let allowed = credential.is_method_allowed(mcp_info.method, mcp_info.target.as_deref());

    if allowed {
        Ok(())
    } else {
        let scope_needed = match mcp_info.method {
            McpMethod::ToolsList => "tools:list or tools:*",
            McpMethod::ToolsCall => {
                if let Some(ref tool) = mcp_info.target {
                    return Err(ProxyError::Forbidden(format!(
                        "Access token lacks permission for tools:call:{} (need tools:call, tools:call:{}, or tools:*)",
                        tool, tool
                    )));
                }
                "tools:call or tools:*"
            }
            McpMethod::ResourcesList => "resources:list or resources:*",
            McpMethod::ResourcesRead => {
                if let Some(ref uri) = mcp_info.target {
                    return Err(ProxyError::Forbidden(format!(
                        "Access token lacks permission for resources:read:{} (need resources:read, resources:read:{}, or resources:*)",
                        uri, uri
                    )));
                }
                "resources:read or resources:*"
            }
            McpMethod::PromptsList => "prompts:list or prompts:*",
            McpMethod::PromptsGet => {
                if let Some(ref name) = mcp_info.target {
                    return Err(ProxyError::Forbidden(format!(
                        "Access token lacks permission for prompts:get:{} (need prompts:get, prompts:get:{}, or prompts:*)",
                        name, name
                    )));
                }
                "prompts:get or prompts:*"
            }
            McpMethod::Unknown => return Ok(()),
        };

        Err(ProxyError::Forbidden(format!(
            "Access token lacks required scope: {}",
            scope_needed
        )))
    }
}

/// Check if request is for SSE (Server-Sent Events)
fn is_sse_request(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/event-stream"))
        .unwrap_or(false)
}

/// Per-request context needed to build sanitized upstream headers and apply
/// tools/list optimizations.
struct ForwardContext<'a> {
    /// Whether NodeFlare validated the credential (so the client `Authorization`
    /// should be stripped) vs. the upstream doing its own auth (forward it).
    auth_enabled: bool,
    client_ip: &'a str,
    host: &'a str,
    /// Server whose tools/list we may filter/slim and whose tool catalog we update.
    server_id: Uuid,
    /// Filter tools/list down to tools the credential may call.
    filter_by_scope: bool,
    /// Trim verbose tool schemas in tools/list.
    slim: bool,
    /// Collapse tools/list into search_tools + call_tool meta-tools.
    search_mode: bool,
}

/// Hop-by-hop headers (RFC 7230 §6.1) — must not be forwarded by a proxy.
const HOP_BY_HOP_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Build the sanitized header set to send upstream: drop hop-by-hop headers (and any
/// header named in the inbound `Connection` token list), strip the client
/// `Authorization` only when NodeFlare did the auth, and add `X-Forwarded-*`.
fn build_upstream_headers(inbound: &axum::http::HeaderMap, fwd: &ForwardContext) -> axum::http::HeaderMap {
    // Headers explicitly listed as connection-tokens must also be dropped.
    let mut conn_tokens: Vec<String> = Vec::new();
    if let Some(conn) = inbound.get("connection").and_then(|v| v.to_str().ok()) {
        for tok in conn.split(',') {
            let t = tok.trim().to_ascii_lowercase();
            if !t.is_empty() {
                conn_tokens.push(t);
            }
        }
    }

    let mut out = axum::http::HeaderMap::new();
    for (name, value) in inbound.iter() {
        let lname = name.as_str().to_ascii_lowercase();
        // `host` is set by the HTTP client from the target URL; `x-forwarded-for` is
        // rebuilt below to append our hop.
        if lname == "host" || lname == "x-forwarded-for" {
            continue;
        }
        if HOP_BY_HOP_HEADERS.contains(&lname.as_str()) {
            continue;
        }
        if conn_tokens.iter().any(|t| t == &lname) {
            continue;
        }
        // Strip the client's Authorization only when we authenticated the request.
        // When the upstream does its own auth (auth_enabled=false), forward it.
        if lname == "authorization" && fwd.auth_enabled {
            continue;
        }
        out.append(name.clone(), value.clone());
    }

    // X-Forwarded-For: append our observed client IP to any existing chain.
    let xff = match inbound.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        Some(existing) if !existing.trim().is_empty() => format!("{}, {}", existing, fwd.client_ip),
        _ => fwd.client_ip.to_string(),
    };
    if let Ok(v) = HeaderValue::from_str(&xff) {
        out.insert("x-forwarded-for", v);
    }
    if let Ok(v) = HeaderValue::from_str(fwd.host) {
        out.insert("x-forwarded-host", v);
    }
    out.insert("x-forwarded-proto", HeaderValue::from_static("https"));

    out
}

/// Caller-identity component of the response cache key.
///
/// Cached list responses (tools/list, resources/list, prompts/list) can vary by who
/// is asking: an upstream MCP server may filter tools by the caller's OAuth scope,
/// and our own future per-credential filtering will too. The cache key must therefore
/// include the caller's authorization context — otherwise one caller's cached (or
/// in-flight coalesced) list could be served to another, an information leak.
///
/// - Authenticated by NodeFlare: we strip the client `Authorization` before
///   forwarding, so the upstream is credential-agnostic today. We still key by our
///   credential id so distinct credentials never share an entry — correct now, and
///   still correct once we filter the list per credential.
/// - Pass-through (upstream does its own auth): the client's credentials are
///   forwarded, so the list can differ per caller. Key on the forwarded
///   credential-bearing headers. (Localization headers and exotic custom auth headers
///   are intentionally not included — a known long-tail limitation, bounded by TTL.)
fn cache_identity(credential: Option<&AuthCredential>, inbound_headers: &axum::http::HeaderMap) -> Vec<u8> {
    if let Some(cred) = credential {
        return cred.id().as_bytes().to_vec();
    }
    let mut identity = Vec::new();
    for name in ["authorization", "cookie"] {
        if let Some(value) = inbound_headers.get(name) {
            identity.extend_from_slice(name.as_bytes());
            identity.push(b'=');
            identity.extend_from_slice(value.as_bytes());
            identity.push(b'\n');
        }
    }
    identity
}

/// Canonical cache-key body: strip the volatile JSON-RPC `id` and `jsonrpc` fields so
/// two otherwise-identical requests (differing only by id) share one cache entry.
/// (serde_json's Value map orders keys, so the encoding is stable.)
fn jsonrpc_cache_key_body(body: &[u8]) -> Vec<u8> {
    if let Ok(mut v) = serde_json::from_slice::<serde_json::Value>(body) {
        if let Some(obj) = v.as_object_mut() {
            obj.remove("id");
            obj.remove("jsonrpc");
            if let Ok(s) = serde_json::to_vec(&v) {
                return s;
            }
        }
    }
    body.to_vec()
}

/// Strip the `id` from a JSON-RPC response so the cached copy is id-agnostic.
fn strip_jsonrpc_id(body: &[u8]) -> Vec<u8> {
    if let Ok(mut v) = serde_json::from_slice::<serde_json::Value>(body) {
        if let Some(obj) = v.as_object_mut() {
            if obj.remove("id").is_some() {
                if let Ok(s) = serde_json::to_vec(&v) {
                    return s;
                }
            }
        }
    }
    body.to_vec()
}

/// Re-inject the caller's JSON-RPC `id` into a cached (id-stripped) response body.
fn inject_jsonrpc_id(body: &[u8], id: Option<&serde_json::Value>) -> Vec<u8> {
    if let Ok(mut v) = serde_json::from_slice::<serde_json::Value>(body) {
        if let Some(obj) = v.as_object_mut() {
            obj.insert(
                "id".to_string(),
                id.cloned().unwrap_or(serde_json::Value::Null),
            );
            if let Ok(s) = serde_json::to_vec(&v) {
                return s;
            }
        }
    }
    body.to_vec()
}

/// Forward a request to the upstream MCP server.
///
/// `credential` is `Some` when NodeFlare auth is enabled (scope checks apply) and
/// `None` when the upstream handles its own auth. This single function replaces the
/// previously-duplicated `forward_request` / `forward_request_no_auth`.
async fn forward_request(
    state: &ProxyState,
    target_url: &str,
    request: Request<Body>,
    credential: Option<&AuthCredential>,
    fwd: &ForwardContext<'_>,
) -> Result<(Response, McpRequestInfo), ProxyError> {
    let method = request.method().clone();
    let inbound_headers = request.headers().clone();
    let is_sse = is_sse_request(&inbound_headers);

    // Sanitize headers once (hop-by-hop strip, Authorization policy, X-Forwarded-*).
    let mut headers = build_upstream_headers(&inbound_headers, fwd);

    // Read body (limit comes from configuration, not a hard-coded constant).
    let mut body_bytes = axum::body::to_bytes(request.into_body(), state.body_limit)
        .await
        .map_err(|e| ProxyError::BadRequest(format!("Failed to read body: {}", e)))?;

    tracing::info!("forward_request: method={}, is_sse={}, auth={}", method, is_sse, fwd.auth_enabled);
    // Request body may contain sensitive data (tool args, secrets, PII) — log only at DEBUG
    if tracing::enabled!(tracing::Level::DEBUG) {
        let req_body_preview = if body_bytes.len() > 500 {
            format!("{}... (truncated)", String::from_utf8_lossy(&body_bytes[..500]))
        } else {
            String::from_utf8_lossy(&body_bytes).to_string()
        };
        tracing::debug!("forward_request: request_body={}", req_body_preview);
    }

    // Extract MCP request info (method + target + id)
    let mut mcp_info = extract_mcp_request_info(&body_bytes);

    // Search mode: handle the synthetic meta-tools before scope checks / forwarding.
    // `target` is cloned so the match doesn't borrow `mcp_info` (we move/return it below).
    let target = mcp_info.target.clone();
    if fwd.search_mode && matches!(mcp_info.method, McpMethod::ToolsCall) {
        match target.as_deref() {
            // `search_tools` is served locally from the tool catalog — never forwarded.
            Some(meta_tools::SEARCH_TOOLS) => {
                let query = meta_tools::extract_search_query(&body_bytes);
                let response =
                    search_tools_response(state, credential, fwd, &query, mcp_info.id.as_ref()).await;
                return Ok((response, mcp_info));
            }
            // `call_tool` unwraps into a real tools/call, then flows through the normal
            // path below (scope-checked against the real tool name, then forwarded).
            Some(meta_tools::CALL_TOOL) => {
                if let Some(rewritten) = meta_tools::rewrite_call_tool_body(&body_bytes) {
                    body_bytes = Bytes::from(rewritten);
                    mcp_info = extract_mcp_request_info(&body_bytes);
                }
            }
            _ => {}
        }
    }

    // Check scope permission before forwarding (only when we did the auth).
    if let Some(cred) = credential {
        if let Err(e) = check_scope_permission(cred, &mcp_info) {
            // Echo the inbound request id so the client can correlate the error.
            return Ok((error_response_with_id(e, mcp_info.id.as_ref()), mcp_info));
        }
    }

    // Session affinity: pin a stateful session to the Fly Machine that owns it.
    let is_initialize = mcp_info.method_str.as_deref() == Some("initialize");
    let affinity = affinity::decide(state, target_url, &headers, is_initialize).await;
    if let Some(machine) = affinity.forced_machine.as_deref() {
        if let Ok(value) = HeaderValue::from_str(machine) {
            headers.insert("fly-force-instance-id", value);
            tracing::info!("affinity: forcing instance {}", machine);
        }
    }

    // tools/list gets token-reduction transforms (scope filter + optional slim) and
    // populates the server's tool catalog, so it must be buffered rather than streamed.
    let is_tools_list = matches!(mcp_info.method, McpMethod::ToolsList);

    // SSE requests: stream directly (no buffering, minimal latency) — except tools/list,
    // which we buffer below to transform it. tools/list is request/response, not a
    // long-lived stream, so buffering it is safe.
    if is_sse && !is_tools_list {
        tracing::info!("SSE request detected, using streaming forward to {}", target_url);
        let response = execute_streaming_request(state, target_url, method, &headers, body_bytes).await?;
        let session_id = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok());
        affinity::capture_session(state, &affinity, session_id).await;
        return Ok((response, mcp_info));
    }

    // tools/list over SSE: buffer (don't cache — the JSON-RPC id lives inside the SSE
    // `data:` frame, which our id-agnostic cache can't rewrite), then catalog + transform.
    if is_sse && is_tools_list {
        let (response_body, status, response_headers) =
            execute_upstream_request(state, target_url, method, &headers, body_bytes).await?;
        if status >= 200 && status < 300 {
            spawn_catalog_update(state, fwd.server_id, &response_headers, &response_body);
        }
        let body = transform_list_body(&mcp_info, &response_headers, response_body, credential, fwd);
        let response = build_response(status, &response_headers, body)?;
        return Ok((response, mcp_info));
    }

    // Only cache read-only MCP methods (list operations)
    let is_cacheable = matches!(
        mcp_info.method,
        McpMethod::ToolsList | McpMethod::ResourcesList | McpMethod::PromptsList
    );

    // Try request coalescing + caching for cacheable requests.
    // The cache is keyed on an id-stripped body and stored without an id, so cache
    // hits never leak another caller's JSON-RPC id — we re-inject the caller's id.
    if is_cacheable {
        let key_body = jsonrpc_cache_key_body(&body_bytes);
        let identity = cache_identity(credential, &inbound_headers);
        match state.request_cache.try_coalesce(target_url, &identity, &key_body).await {
            CoalesceResult::Cached(cached) => {
                tracing::debug!("Cache hit for {}", target_url);
                let body = inject_jsonrpc_id(&cached.body, mcp_info.id.as_ref());
                let body = transform_list_body(&mcp_info, &cached.headers, body, credential, fwd);
                let response = build_response(cached.status, &cached.headers, body)?;
                return Ok((response, mcp_info));
            }
            CoalesceResult::Coalesced(cached) => {
                tracing::debug!("Request coalesced for {}", target_url);
                let body = inject_jsonrpc_id(&cached.body, mcp_info.id.as_ref());
                let body = transform_list_body(&mcp_info, &cached.headers, body, credential, fwd);
                let response = build_response(cached.status, &cached.headers, body)?;
                return Ok((response, mcp_info));
            }
            CoalesceResult::Execute(handle) => {
                // Execute the request and cache the result
                match execute_upstream_request(state, target_url, method, &headers, body_bytes).await {
                    Ok((response_body, status, response_headers)) => {
                        // Cache successful responses only, storing an id-stripped copy.
                        // The cached body is the RAW upstream response; per-caller
                        // transforms are applied on egress below, so the cache stays
                        // reusable and never stores a filtered/slimmed copy.
                        if status >= 200 && status < 300 {
                            spawn_catalog_update(state, fwd.server_id, &response_headers, &response_body);
                            let cacheable = strip_jsonrpc_id(&response_body);
                            state.request_cache.complete(handle, cacheable, status, response_headers.clone()).await;
                        } else {
                            state.request_cache.cancel(handle).await;
                        }

                        let body = transform_list_body(&mcp_info, &response_headers, response_body, credential, fwd);
                        let response = build_response(status, &response_headers, body)?;
                        return Ok((response, mcp_info));
                    }
                    Err(e) => {
                        state.request_cache.cancel(handle).await;
                        return Err(e);
                    }
                }
            }
        }
    }

    // Non-cacheable requests: execute directly
    let (response_body, status, response_headers) =
        execute_upstream_request(state, target_url, method, &headers, body_bytes).await?;

    let session_id = response_headers
        .iter()
        .find(|(k, _)| k == "mcp-session-id")
        .map(|(_, v)| v.as_str());
    affinity::capture_session(state, &affinity, session_id).await;

    let response = build_response(status, &response_headers, response_body)?;
    Ok((response, mcp_info))
}

/// Content-type from a response header list, if present.
fn response_content_type(headers: &[(String, String)]) -> Option<&str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str())
}

/// Apply tools/list token-reduction transforms to an outgoing response body.
/// No-op for any method other than tools/list, and when neither transform is active.
fn transform_list_body(
    mcp_info: &McpRequestInfo,
    headers: &[(String, String)],
    body: Vec<u8>,
    credential: Option<&AuthCredential>,
    fwd: &ForwardContext<'_>,
) -> Vec<u8> {
    if !matches!(mcp_info.method, McpMethod::ToolsList) {
        return body;
    }
    let content_type = response_content_type(headers);
    // Search mode replaces the whole tool surface with the two meta-tools.
    if fwd.search_mode {
        return mcp_transform::replace_tools(&body, content_type, &meta_tools::definitions());
    }
    // Filtering only happens when we authenticated the caller (credential present);
    // skip the work entirely when nothing would change.
    let will_filter = fwd.filter_by_scope && credential.is_some();
    if !will_filter && !fwd.slim {
        return body;
    }
    mcp_transform::transform_tools_list(&body, content_type, credential, fwd.filter_by_scope, fwd.slim)
}

/// Serve a `search_tools` call locally from the server's tool catalog. Uses semantic
/// (embedding) search when available, falling back to lexical search; honors scope
/// filtering so a credential only discovers tools it is allowed to call.
async fn search_tools_response(
    state: &ProxyState,
    credential: Option<&AuthCredential>,
    fwd: &ForwardContext<'_>,
    query: &str,
    id: Option<&serde_json::Value>,
) -> Response {
    let matched = search_catalog(state, credential, fwd, query).await;
    let refs: Vec<&Tool> = matched.iter().collect();
    let body = meta_tools::search_result_json(&refs, id);
    let headers = vec![("content-type".to_string(), "application/json".to_string())];
    build_response(200, &headers, body)
        .unwrap_or_else(|_| Response::builder().status(500).body(Body::empty()).unwrap())
}

/// Keep only tools the credential is allowed to call (when scope filtering is on).
fn scope_filter_tools(
    tools: Vec<Tool>,
    credential: Option<&AuthCredential>,
    fwd: &ForwardContext<'_>,
) -> Vec<Tool> {
    match (fwd.filter_by_scope, credential) {
        (true, Some(cred)) => tools
            .into_iter()
            .filter(|t| cred.is_method_allowed(McpMethod::ToolsCall, Some(&t.name)))
            .collect(),
        _ => tools,
    }
}

/// Find catalog tools matching `query`: semantic search first (if embeddings are
/// configured and the query is non-empty), otherwise lexical. Always scope-filtered.
async fn search_catalog(
    state: &ProxyState,
    credential: Option<&AuthCredential>,
    fwd: &ForwardContext<'_>,
    query: &str,
) -> Vec<Tool> {
    let limit = meta_tools::search_limit();

    // Semantic path.
    if let Some(client) = &state.embedding {
        if !query.trim().is_empty() {
            if let Some(qvec) = client.embed(query).await {
                // Over-fetch so scope filtering still leaves enough results.
                let fetch = (limit * 4) as i64;
                match ToolRepository::search_by_embedding(&state.db, fwd.server_id, qvec, fetch).await {
                    Ok(tools) if !tools.is_empty() => {
                        let filtered = scope_filter_tools(tools, credential, fwd);
                        if !filtered.is_empty() {
                            return filtered.into_iter().take(limit).collect();
                        }
                    }
                    Ok(_) => {} // no embedded tools yet — fall back to lexical
                    Err(e) => tracing::warn!("semantic search failed for {}: {}", fwd.server_id, e),
                }
            }
        }
    }

    // Lexical fallback.
    let tools = ToolRepository::list_by_server(&state.db, fwd.server_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!("search_tools: catalog read failed for {}: {}", fwd.server_id, e);
            Vec::new()
        });
    let visible = scope_filter_tools(tools, credential, fwd);
    meta_tools::rank_tools(&visible, query, limit)
        .into_iter()
        .cloned()
        .collect()
}

/// Best-effort, non-blocking refresh of the server's tool catalog from an observed
/// tools/list response. For servers whose list varies by caller (e.g. OAuth scope in
/// pass-through mode) this records the most-recently observed set. An empty list is
/// ignored so a low-scope caller can't wipe a populated catalog.
fn spawn_catalog_update(
    state: &ProxyState,
    server_id: Uuid,
    headers: &[(String, String)],
    body: &[u8],
) {
    let content_type = response_content_type(headers);
    let Some(tools) = mcp_transform::extract_tools(body, content_type) else {
        return;
    };
    if tools.is_empty() {
        return;
    }
    let db = state.db.clone();
    let embedding = state.embedding.clone();
    tokio::spawn(async move {
        let upserts: Vec<UpsertTool> = tools
            .into_iter()
            .map(|t| UpsertTool {
                server_id,
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
            })
            .collect();
        if let Err(e) = ToolRepository::sync_tools(&db, server_id, upserts).await {
            tracing::warn!("tool catalog sync failed for server {}: {}", server_id, e);
            return;
        }
        // Backfill embeddings for new/changed tools (sync nulls embeddings on change).
        if let Some(client) = embedding {
            backfill_embeddings(&db, &client, server_id).await;
        }
    });
}

/// Text used to embed a tool: name plus description (when present).
fn tool_embed_text(tool: &Tool) -> String {
    match &tool.description {
        Some(desc) if !desc.is_empty() => format!("{}: {}", tool.name, desc),
        _ => tool.name.clone(),
    }
}

/// Embed and store vectors for a server's tools that don't have one yet. Best-effort.
async fn backfill_embeddings(
    db: &mcp_db::DbPool,
    client: &embedding::EmbeddingClient,
    server_id: Uuid,
) {
    let missing = match ToolRepository::list_missing_embeddings(db, server_id).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("list_missing_embeddings failed for {}: {}", server_id, e);
            return;
        }
    };
    if missing.is_empty() {
        return;
    }
    let texts: Vec<String> = missing.iter().map(tool_embed_text).collect();
    let Some(vectors) = client.embed_batch(&texts).await else {
        return;
    };
    for (tool, vector) in missing.iter().zip(vectors.into_iter()) {
        if let Some(vector) = vector {
            if let Err(e) = ToolRepository::set_embedding(db, tool.id, vector).await {
                tracing::warn!("set_embedding failed for tool {}: {}", tool.id, e);
            }
        }
    }
}

/// Build response from raw parts
fn build_response(status: u16, headers: &[(String, String)], body: Vec<u8>) -> Result<Response, ProxyError> {
    let mut builder = Response::builder().status(status);
    for (name, value) in headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    builder
        .body(Body::from(body))
        .map_err(|e| ProxyError::Internal(e.to_string()))
}

/// Execute request to upstream MCP server
async fn execute_upstream_request(
    state: &ProxyState,
    target_url: &str,
    method: axum::http::Method,
    headers: &axum::http::HeaderMap,
    body_bytes: Bytes,
) -> Result<(Vec<u8>, u16, Vec<(String, String)>), ProxyError> {
    // Build outgoing request. `headers` is already sanitized by build_upstream_headers
    // (hop-by-hop stripped, Authorization policy applied, X-Forwarded-* added).
    let mut req_builder = state.http_client.request(method, target_url);
    for (name, value) in headers.iter() {
        req_builder = req_builder.header(name, value);
    }

    req_builder = req_builder.body(body_bytes);

    // Send request
    let response = req_builder
        .send()
        .await
        .map_err(|e| ProxyError::ServiceUnavailable(format!("Upstream error: {}", e)))?;

    // Convert response
    let status = response.status().as_u16();

    // Only preserve essential headers to reduce allocations
    // Most headers (connection, transfer-encoding, etc.) are handled by the framework
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .filter_map(|(k, v)| {
            // Only keep headers that are meaningful for the response
            let name = k.as_str();
            match name {
                "content-type" | "content-encoding" | "cache-control" | "etag" | "vary" | "x-request-id" | "mcp-session-id" => {
                    v.to_str().ok().map(|val| (name.to_string(), val.to_string()))
                }
                _ => None,
            }
        })
        .collect();

    let body = response
        .bytes()
        .await
        .map_err(|e| ProxyError::Internal(format!("Failed to read response: {}", e)))?;

    tracing::info!("upstream response: status={}", status);
    // Response body may contain sensitive data — log only at DEBUG
    if tracing::enabled!(tracing::Level::DEBUG) {
        let body_preview = if body.len() > 500 {
            format!("{}... (truncated, total {} bytes)", String::from_utf8_lossy(&body[..500]), body.len())
        } else {
            String::from_utf8_lossy(&body).to_string()
        };
        tracing::debug!("upstream response: body={}", body_preview);
    }

    Ok((body.to_vec(), status, headers))
}

/// Execute streaming request for SSE (Server-Sent Events)
/// Streams response directly without buffering for minimal latency
async fn execute_streaming_request(
    state: &ProxyState,
    target_url: &str,
    method: axum::http::Method,
    headers: &axum::http::HeaderMap,
    body_bytes: Bytes,
) -> Result<Response, ProxyError> {
    // Reuse the shared, long-lived SSE client (built once at startup with a
    // connect_timeout + read/idle timeout and no total timeout) so connections are
    // pooled across requests instead of rebuilt per request.
    let mut req_builder = state.sse_client.request(method, target_url);

    // `headers` is already sanitized (hop-by-hop stripped, Authorization policy
    // applied, X-Forwarded-* added), and still carries Accept for SSE.
    for (name, value) in headers.iter() {
        req_builder = req_builder.header(name, value);
    }

    req_builder = req_builder.body(body_bytes);

    // Send request
    let response = req_builder
        .send()
        .await
        .map_err(|e| ProxyError::ServiceUnavailable(format!("Upstream error: {}", e)))?;

    let status = response.status();

    // Build response with streaming body
    let mut builder = Response::builder().status(status);

    // Copy essential headers from upstream - don't override content-type!
    let mut has_content_type = false;
    for (name, value) in response.headers().iter() {
        let header_name = name.as_str();
        match header_name {
            "content-type" => {
                if let Ok(val) = value.to_str() {
                    builder = builder.header(header_name, val);
                    has_content_type = true;
                    tracing::debug!("SSE streaming: preserving upstream content-type: {}", val);
                }
            }
            "content-encoding" | "cache-control" | "x-request-id" | "mcp-session-id" | "x-accel-buffering" => {
                if let Ok(val) = value.to_str() {
                    builder = builder.header(header_name, val);
                }
            }
            _ => {}
        }
    }

    // Only set default headers if upstream didn't provide them
    if !has_content_type {
        builder = builder.header("content-type", "text/event-stream");
    }
    builder = builder.header("cache-control", "no-cache");
    builder = builder.header("x-accel-buffering", "no"); // Disable proxy buffering

    // Stream the response body directly without buffering
    let stream = response.bytes_stream().map(|result| {
        result.map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        })
    });

    let body = Body::from_stream(stream);

    builder
        .body(body)
        .map_err(|e| ProxyError::Internal(e.to_string()))
}

/// Extract server slug from subdomain
/// e.g., "my-server.mcp.cloud" with base "mcp.cloud" -> "my-server"
/// e.g., "my-server.localhost:8081" with base "localhost:8081" -> "my-server"
fn extract_subdomain(host: &str, base_domain: &str) -> Result<String, ProxyError> {
    // Remove port from host if present for comparison
    let host_without_port = host.split(':').next().unwrap_or(host);
    let base_without_port = base_domain.split(':').next().unwrap_or(base_domain);

    // Check if this is a subdomain of the base domain
    if let Some(subdomain) = host_without_port.strip_suffix(&format!(".{}", base_without_port)) {
        if subdomain.is_empty() || subdomain.contains('.') {
            return Err(ProxyError::BadRequest("Invalid subdomain format".into()));
        }
        Ok(subdomain.to_string())
    } else if host_without_port == base_without_port {
        // Direct access to base domain (e.g., mcp.cloud without subdomain)
        Err(ProxyError::BadRequest(
            "No server specified. Use {server-slug}.{base-domain}".into(),
        ))
    } else {
        Err(ProxyError::BadRequest(format!(
            "Invalid host: expected *.{}",
            base_domain
        )))
    }
}

#[derive(Debug)]
pub enum ProxyError {
    Unauthorized(String, Option<String>), // message, optional host for WWW-Authenticate
    Forbidden(String),
    NotFound(String),
    BadRequest(String),
    RateLimitExceeded,
    QuotaExceeded(String),
    PaymentRequired(String),
    ServiceUnavailable(String),
    Internal(String),
}

impl std::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyError::Unauthorized(m, _) => write!(f, "Unauthorized: {}", m),
            ProxyError::Forbidden(m) => write!(f, "Forbidden: {}", m),
            ProxyError::NotFound(m) => write!(f, "Not found: {}", m),
            ProxyError::BadRequest(m) => write!(f, "Bad request: {}", m),
            ProxyError::RateLimitExceeded => write!(f, "Rate limit exceeded"),
            ProxyError::QuotaExceeded(m) => write!(f, "Quota exceeded: {}", m),
            ProxyError::PaymentRequired(m) => write!(f, "Payment required: {}", m),
            ProxyError::ServiceUnavailable(m) => write!(f, "Service unavailable: {}", m),
            ProxyError::Internal(m) => write!(f, "Internal error: {}", m),
        }
    }
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        // No request context here, so the id can't be correlated; callers that know
        // the inbound JSON-RPC id use `error_response_with_id` instead.
        error_response_with_id(self, None)
    }
}

/// Render a `ProxyError` as a JSON-RPC 2.0 error, echoing the caller's request `id`
/// when known (so clients can correlate the error with their request).
fn error_response_with_id(err: ProxyError, id: Option<&serde_json::Value>) -> Response {
    {
        let (status, message, error_code, www_authenticate) = match &err {
            ProxyError::Unauthorized(m, host) => {
                // Include WWW-Authenticate header for OAuth 2.1 compliance (RFC 9728)
                let resource_metadata_url = match host {
                    Some(h) => format!("https://{}/.well-known/oauth-protected-resource", h),
                    None => "/.well-known/oauth-protected-resource".to_string(),
                };
                let www_auth = format!(
                    "Bearer resource_metadata=\"{}\", scope=\"*\"",
                    resource_metadata_url
                );
                (StatusCode::UNAUTHORIZED, m.clone(), "UNAUTHORIZED", Some(www_auth))
            }
            ProxyError::Forbidden(m) => (StatusCode::FORBIDDEN, m.clone(), "FORBIDDEN", None),
            ProxyError::NotFound(m) => (StatusCode::NOT_FOUND, m.clone(), "NOT_FOUND", None),
            ProxyError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone(), "BAD_REQUEST", None),
            ProxyError::RateLimitExceeded => {
                (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded. Please slow down.".to_string(), "RATE_LIMIT_EXCEEDED", None)
            }
            ProxyError::QuotaExceeded(m) => {
                (StatusCode::TOO_MANY_REQUESTS, m.clone(), "MONTHLY_QUOTA_EXCEEDED", None)
            }
            ProxyError::PaymentRequired(m) => {
                (StatusCode::PAYMENT_REQUIRED, m.clone(), "PAYMENT_REQUIRED", None)
            }
            ProxyError::ServiceUnavailable(m) => {
                // Log internal details, return safe message
                tracing::error!("Service unavailable: {}", m);
                (StatusCode::SERVICE_UNAVAILABLE, "Service temporarily unavailable".to_string(), "SERVICE_UNAVAILABLE", None)
            }
            ProxyError::Internal(m) => {
                // Log internal details, return safe message
                tracing::error!("Internal proxy error: {}", m);
                (StatusCode::INTERNAL_SERVER_ERROR, "An internal error occurred".to_string(), "INTERNAL_ERROR", None)
            }
        };

        // Map HTTP status to JSON-RPC 2.0 error codes
        // -32700: Parse error, -32600: Invalid Request, -32601: Method not found
        // -32602: Invalid params, -32603: Internal error
        // -32000 to -32099: Server error (implementation-defined)
        let jsonrpc_code = match status {
            StatusCode::BAD_REQUEST => -32600,      // Invalid Request
            StatusCode::UNAUTHORIZED => -32001,     // Server error: unauthorized
            StatusCode::FORBIDDEN => -32002,        // Server error: forbidden
            StatusCode::NOT_FOUND => -32601,        // Method not found (closest match)
            StatusCode::TOO_MANY_REQUESTS => -32003, // Server error: rate limit
            StatusCode::PAYMENT_REQUIRED => -32004, // Server error: payment required
            StatusCode::SERVICE_UNAVAILABLE => -32005, // Server error: service unavailable
            _ => -32603,                            // Internal error
        };

        // JSON-RPC 2.0 compliant error response (echo the inbound id when known)
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id.cloned().unwrap_or(serde_json::Value::Null),
            "error": {
                "code": jsonrpc_code,
                "message": message,
                "data": {
                    "type": error_code,
                    "status": status.as_u16()
                }
            }
        });

        let mut response = (status, axum::Json(body)).into_response();

        // Add WWW-Authenticate header for 401 responses (OAuth 2.1 compliance)
        if let Some(www_auth) = www_authenticate {
            response.headers_mut().insert(
                axum::http::header::WWW_AUTHENTICATE,
                www_auth.parse().unwrap_or_else(|_| HeaderValue::from_static("Bearer")),
            );
        }

        response
    }
}
