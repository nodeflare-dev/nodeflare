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
use mcp_db::{McpServer, ServerRepository};
use auth::AuthCredential;
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tower_http::{limit::RequestBodyLimitLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod cache;
mod rate_limit;
mod redis_cache;

use cache::{RequestCache, CoalesceResult};
use redis_cache::RedisCache;

pub struct ProxyState {
    pub config: AppConfig,
    pub db: mcp_db::DbPool,
    pub redis: fred::prelude::RedisClient,
    pub http_client: reqwest::Client,
    pub request_cache: RequestCache,
    pub redis_cache: RedisCache,
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

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
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

    let state = Arc::new(ProxyState {
        config: config.clone(),
        db: db_pool,
        redis,
        http_client,
        request_cache,
        redis_cache,
    });

    // Get request body limit from env (default: 10MB for proxy)
    let body_limit: usize = std::env::var("PROXY_BODY_LIMIT_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10 * 1024 * 1024); // 10MB default for proxy

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
) -> Json<OAuthServerMetadata> {
    tracing::info!("OAuth metadata requested from proxy");

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
    let is_browser = request.method() == axum::http::Method::GET
        && request
            .headers()
            .get(axum::http::header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .map(|accept| accept.contains("text/html"))
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

    // 4. Check monthly quota based on workspace plan - graceful degradation on Redis errors
    // (This applies regardless of auth_enabled to prevent abuse)
    tracing::debug!("proxy_handler: checking monthly quota");
    match rate_limit::check_monthly_quota(&state, server.workspace_id).await {
        Ok(_) => tracing::debug!("proxy_handler: monthly quota check passed"),
        Err(ProxyError::QuotaExceeded(msg)) => return Err(ProxyError::QuotaExceeded(msg)),
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

    // 6. Forward request (with or without scope check depending on auth)
    let (response, mcp_info) = if let Some(ref cred) = credential {
        forward_request(&state, &target_url, request, cred).await?
    } else {
        forward_request_no_auth(&state, &target_url, request).await?
    };
    tracing::info!("proxy_handler: request forwarded, status={}", response.status());

    // 7. Increment monthly counter on success (async, don't block response)
    if response.status().is_success() {
        let state_clone = state.clone();
        let workspace_id = server.workspace_id;
        tokio::spawn(async move {
            if let Err(e) = rate_limit::increment_monthly_counter(&state_clone, workspace_id).await {
                tracing::warn!("Failed to increment monthly counter: {}", e);
            }
        });
    }

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

async fn resolve_server(state: &ProxyState, slug: &str) -> Result<McpServer, ProxyError> {
    tracing::debug!("resolve_server: looking up slug={}", slug);

    // Try Redis cache first
    let cache_start = Instant::now();
    if let Some(cached) = state.redis_cache.get_server(slug).await {
        tracing::debug!("resolve_server: cache HIT for slug={} (took {:?})", slug, cache_start.elapsed());
        return Ok(cached.to_mcp_server());
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

    Ok(server)
}

/// Extracted MCP request info for scope checking and logging
#[derive(Debug, Clone)]
struct McpRequestInfo {
    method: McpMethod,
    method_str: Option<String>,
    target: Option<String>,
}

/// Extract MCP method and target from JSON-RPC request body
fn extract_mcp_request_info(body: &[u8]) -> McpRequestInfo {
    let mut info = McpRequestInfo {
        method: McpMethod::Unknown,
        method_str: None,
        target: None,
    };

    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
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

async fn forward_request(
    state: &ProxyState,
    target_url: &str,
    request: Request<Body>,
    credential: &AuthCredential,
) -> Result<(Response, McpRequestInfo), ProxyError> {
    let method = request.method().clone();
    let headers = request.headers().clone();
    let is_sse = is_sse_request(&headers);

    // Read body
    let body_bytes = axum::body::to_bytes(request.into_body(), 10 * 1024 * 1024)
        .await
        .map_err(|e| ProxyError::BadRequest(format!("Failed to read body: {}", e)))?;

    // Log request body for debugging
    let req_body_preview = if body_bytes.len() > 500 {
        format!("{}... (truncated)", String::from_utf8_lossy(&body_bytes[..500]))
    } else {
        String::from_utf8_lossy(&body_bytes).to_string()
    };
    tracing::info!("forward_request: method={}, is_sse={}, request_body={}", method, is_sse, req_body_preview);

    // Extract MCP request info (method + target)
    let mcp_info = extract_mcp_request_info(&body_bytes);

    // Check scope permission before forwarding
    check_scope_permission(credential, &mcp_info)?;

    // SSE requests: use streaming (no buffering, minimal latency)
    if is_sse {
        tracing::info!("SSE request detected, using streaming forward to {}", target_url);
        let response = execute_streaming_request(state, target_url, method, &headers, body_bytes).await?;
        return Ok((response, mcp_info));
    }

    // Only cache read-only MCP methods (list operations)
    let is_cacheable = matches!(
        mcp_info.method,
        McpMethod::ToolsList | McpMethod::ResourcesList | McpMethod::PromptsList
    );

    // Try request coalescing + caching for cacheable requests
    if is_cacheable {
        match state.request_cache.try_coalesce(target_url, &body_bytes).await {
            CoalesceResult::Cached(cached) => {
                tracing::debug!("Cache hit for {}", target_url);
                let response = build_response_from_cache(&cached)?;
                return Ok((response, mcp_info));
            }
            CoalesceResult::Coalesced(cached) => {
                tracing::debug!("Request coalesced for {}", target_url);
                let response = build_response_from_cache(&cached)?;
                return Ok((response, mcp_info));
            }
            CoalesceResult::Execute(handle) => {
                // Execute the request and cache the result
                match execute_upstream_request(state, target_url, method, &headers, body_bytes).await {
                    Ok((response_body, status, response_headers)) => {
                        // Cache successful responses only
                        if status >= 200 && status < 300 {
                            state.request_cache.complete(handle, response_body.clone(), status, response_headers.clone()).await;
                        } else {
                            state.request_cache.cancel(handle).await;
                        }

                        let response = build_response(status, &response_headers, response_body)?;
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

    let response = build_response(status, &response_headers, response_body)?;
    Ok((response, mcp_info))
}

/// Forward request without authentication checks (for servers with auth_enabled=false)
/// The MCP server handles its own authentication
async fn forward_request_no_auth(
    state: &ProxyState,
    target_url: &str,
    request: Request<Body>,
) -> Result<(Response, McpRequestInfo), ProxyError> {
    let method = request.method().clone();
    let headers = request.headers().clone();
    let is_sse = is_sse_request(&headers);

    // Read body
    let body_bytes = axum::body::to_bytes(request.into_body(), 10 * 1024 * 1024)
        .await
        .map_err(|e| ProxyError::BadRequest(format!("Failed to read body: {}", e)))?;

    // Log request body for debugging
    let req_body_preview = if body_bytes.len() > 500 {
        format!("{}... (truncated)", String::from_utf8_lossy(&body_bytes[..500]))
    } else {
        String::from_utf8_lossy(&body_bytes).to_string()
    };
    tracing::info!("forward_request_no_auth: method={}, is_sse={}, request_body={}", method, is_sse, req_body_preview);

    // Extract MCP request info (method + target) for logging purposes
    let mcp_info = extract_mcp_request_info(&body_bytes);

    // Skip scope permission check - auth is handled by the MCP server itself

    // SSE requests: use streaming (no buffering, minimal latency)
    if is_sse {
        tracing::info!("SSE request detected, using streaming forward to {}", target_url);
        let response = execute_streaming_request(state, target_url, method, &headers, body_bytes).await?;
        return Ok((response, mcp_info));
    }

    // Only cache read-only MCP methods (list operations)
    let is_cacheable = matches!(
        mcp_info.method,
        McpMethod::ToolsList | McpMethod::ResourcesList | McpMethod::PromptsList
    );

    // Try request coalescing + caching for cacheable requests
    if is_cacheable {
        match state.request_cache.try_coalesce(target_url, &body_bytes).await {
            CoalesceResult::Cached(cached) => {
                tracing::debug!("Cache hit for {}", target_url);
                let response = build_response_from_cache(&cached)?;
                return Ok((response, mcp_info));
            }
            CoalesceResult::Coalesced(cached) => {
                tracing::debug!("Request coalesced for {}", target_url);
                let response = build_response_from_cache(&cached)?;
                return Ok((response, mcp_info));
            }
            CoalesceResult::Execute(handle) => {
                // Execute the request and cache the result
                match execute_upstream_request(state, target_url, method, &headers, body_bytes).await {
                    Ok((response_body, status, response_headers)) => {
                        // Cache successful responses only
                        if status >= 200 && status < 300 {
                            state.request_cache.complete(handle, response_body.clone(), status, response_headers.clone()).await;
                        } else {
                            state.request_cache.cancel(handle).await;
                        }

                        let response = build_response(status, &response_headers, response_body)?;
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

    let response = build_response(status, &response_headers, response_body)?;
    Ok((response, mcp_info))
}

/// Build response from cached data
fn build_response_from_cache(cached: &cache::CachedResponse) -> Result<Response, ProxyError> {
    let mut builder = Response::builder().status(cached.status);
    for (name, value) in &cached.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    // Bytes implements Into<Body> efficiently without copying
    builder
        .body(Body::from(cached.body.clone()))
        .map_err(|e| ProxyError::Internal(e.to_string()))
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
    // Build outgoing request
    let mut req_builder = state.http_client.request(method, target_url);

    // Copy relevant headers
    for (name, value) in headers.iter() {
        if name != "host" && name != "authorization" {
            req_builder = req_builder.header(name, value);
        }
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

    // Log response body for debugging (truncate if too long)
    let body_preview = if body.len() > 500 {
        format!("{}... (truncated, total {} bytes)", String::from_utf8_lossy(&body[..500]), body.len())
    } else {
        String::from_utf8_lossy(&body).to_string()
    };
    tracing::info!("upstream response: status={}, body={}", status, body_preview);

    Ok((body.to_vec(), status, headers))
}

/// Execute streaming request for SSE (Server-Sent Events)
/// Streams response directly without buffering for minimal latency
async fn execute_streaming_request(
    _state: &ProxyState,
    target_url: &str,
    method: axum::http::Method,
    headers: &axum::http::HeaderMap,
    body_bytes: Bytes,
) -> Result<Response, ProxyError> {
    // Build outgoing request - use a client without timeout for SSE
    let sse_client = reqwest::Client::builder()
        .build()
        .map_err(|e| ProxyError::Internal(format!("Failed to create SSE client: {}", e)))?;

    let mut req_builder = sse_client.request(method, target_url);

    // Copy relevant headers (including Accept for SSE)
    for (name, value) in headers.iter() {
        if name != "host" && name != "authorization" {
            req_builder = req_builder.header(name, value);
        }
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
            "cache-control" | "x-request-id" | "mcp-session-id" | "x-accel-buffering" => {
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
        let (status, message, error_code, www_authenticate) = match &self {
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

        // JSON-RPC 2.0 compliant error response
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": null,
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
