use anyhow::Result;
use apalis::prelude::*;
use apalis_redis::RedisStorage;
use axum::{routing::get, Router};
use mcp_auth::CryptoService;
use mcp_common::{types::LogStream, AppConfig, EventPublisher};
use mcp_db::{DeploymentRepository, ErrorHintRepository, NotificationSettingsRepository, RegionStatus, SecretRepository, ServerRegionRepository, ServerRepository, UpdateDeployment, UpdateServerRegion, UserPreferencesRepository, UserRepository, WorkspaceRepository};
use mcp_email::EmailService;
use mcp_github::GitHubApp;
use mcp_queue::{BuildJob, DeployJob, JobQueue};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod flyctl;
mod flyio;

// Note: docker module kept for reference but not used (using flyctl remote builder instead)
#[allow(dead_code)]
mod docker;

/// Send deployment notification email to workspace owner
async fn send_deploy_notification(
    ctx: &BuilderContext,
    server_id: uuid::Uuid,
    success: bool,
    error_message: Option<&str>,
) {
    let email_service = match &ctx.email {
        Some(s) => s,
        None => return,
    };

    // Get server -> workspace -> owner user
    let server = match ServerRepository::find_by_id(&ctx.db, server_id).await {
        Ok(Some(s)) => s,
        _ => return,
    };

    let workspace = match WorkspaceRepository::find_by_id(&ctx.db, server.workspace_id).await {
        Ok(Some(w)) => w,
        _ => return,
    };

    let owner = match UserRepository::find_by_id(&ctx.db, workspace.owner_id).await {
        Ok(Some(u)) => u,
        _ => return,
    };

    // Check notification settings
    let settings = match NotificationSettingsRepository::get_or_create(&ctx.db, owner.id).await {
        Ok(s) => s,
        _ => return,
    };

    let app_url = std::env::var("APP_URL").unwrap_or_else(|_| "https://mcpcloud.dev".to_string());

    if success && settings.email_deploy_success {
        let deploy_url = format!("{}/dashboard/servers/{}", app_url, server_id);
        if let Err(e) = email_service.send_deploy_success(&owner.email, &server.name, &deploy_url).await {
            tracing::error!("Failed to send deploy success email: {}", e);
        }
    } else if !success && settings.email_deploy_failure {
        let logs_url = format!("{}/dashboard/servers/{}/logs", app_url, server_id);
        let error_msg = error_message.unwrap_or("Unknown error");
        if let Err(e) = email_service.send_deploy_failure(&owner.email, &server.name, error_msg, &logs_url).await {
            tracing::error!("Failed to send deploy failure email: {}", e);
        }
    }
}

/// Get user's locale preference from server_id
async fn get_user_locale_for_server(pool: &mcp_db::DbPool, server_id: uuid::Uuid) -> String {
    // Get server -> workspace -> owner -> preferences -> locale
    let locale = async {
        let server = ServerRepository::find_by_id(pool, server_id).await.ok()??;
        let workspace = WorkspaceRepository::find_by_id(pool, server.workspace_id).await.ok()??;
        let locale = UserPreferencesRepository::get_locale(pool, workspace.owner_id).await.ok()?;
        Some(locale)
    }.await;

    locale.unwrap_or_else(|| "en".to_string())
}

use std::sync::OnceLock;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Preprocessed error hint for efficient matching
/// Scalability: Keywords are pre-lowercased to avoid repeated string operations
#[derive(Clone, Debug)]
struct PreprocessedErrorHint {
    /// Original hint data
    hint_message: String,
    /// Pre-lowercased keywords for efficient matching
    keywords_lower: Vec<String>,
}

impl From<mcp_db::ErrorHint> for PreprocessedErrorHint {
    fn from(hint: mcp_db::ErrorHint) -> Self {
        Self {
            hint_message: hint.hint_message,
            // Pre-process keywords to lowercase once
            keywords_lower: hint.keywords.iter().map(|k| k.to_lowercase()).collect(),
        }
    }
}

/// Cache for error hints - keyed by locale, stores (preprocessed hints, last_updated)
static ERROR_HINTS_CACHE: OnceLock<RwLock<HashMap<String, (Vec<PreprocessedErrorHint>, std::time::Instant)>>> = OnceLock::new();

/// Cache TTL for error hints (5 minutes)
const ERROR_HINTS_CACHE_TTL_SECS: u64 = 300;

/// Maximum cache entries to prevent unbounded memory growth
const MAX_CACHE_LOCALES: usize = 100;

/// Get cached error hints for a locale, fetching from DB if needed
async fn get_cached_error_hints(pool: &mcp_db::DbPool, locale: &str) -> Vec<PreprocessedErrorHint> {
    let cache = ERROR_HINTS_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    let now = std::time::Instant::now();

    // Check if we have valid cached hints
    {
        let cache_read = cache.read().await;
        if let Some((hints, last_updated)) = cache_read.get(locale) {
            if now.duration_since(*last_updated).as_secs() < ERROR_HINTS_CACHE_TTL_SECS {
                return hints.clone();
            }
        }
    }

    // Cache miss or expired - fetch from database
    match ErrorHintRepository::list_active(pool, locale).await {
        Ok(hints) => {
            // Pre-process hints for efficient matching
            let preprocessed: Vec<PreprocessedErrorHint> = hints.into_iter().map(|h| h.into()).collect();

            let mut cache_write = cache.write().await;

            // Scalability: Limit cache size to prevent unbounded memory growth
            if cache_write.len() >= MAX_CACHE_LOCALES {
                // Remove oldest entry (simple LRU approximation)
                if let Some(oldest_key) = cache_write
                    .iter()
                    .min_by_key(|(_, (_, instant))| *instant)
                    .map(|(k, _)| k.clone())
                {
                    cache_write.remove(&oldest_key);
                }
            }

            cache_write.insert(locale.to_string(), (preprocessed.clone(), now));
            preprocessed
        }
        Err(e) => {
            tracing::warn!("Failed to fetch error hints from database: {}", e);
            Vec::new()
        }
    }
}

/// Analyze error message and return user-friendly hints using cached hints
/// Scalability: Keywords are pre-lowercased in cache, error message is lowercased once
async fn analyze_error_for_hints(pool: &mcp_db::DbPool, error_msg: &str, locale: &str) -> Option<String> {
    // Lowercase error message once (not per-keyword)
    let error_lower = error_msg.to_lowercase();

    // First try to find a hint in the user's locale (using cache)
    let hints = get_cached_error_hints(pool, locale).await;

    for hint in &hints {
        // Keywords are already lowercased in cache
        let all_match = hint.keywords_lower.iter().all(|keyword| {
            error_lower.contains(keyword)
        });

        if all_match {
            return Some(format!("\n\n{}", hint.hint_message));
        }
    }

    // If no match found and locale is not 'en', fall back to English
    if locale != "en" {
        let en_hints = get_cached_error_hints(pool, "en").await;

        for hint in en_hints {
            // Keywords are already lowercased in cache
            let all_match = hint.keywords_lower.iter().all(|keyword| {
                error_lower.contains(keyword)
            });

            if all_match {
                return Some(format!("\n\n{}", hint.hint_message));
            }
        }
    }

    None
}

struct BuilderContext {
    config: AppConfig,
    db: mcp_db::DbPool,
    #[allow(dead_code)]
    job_queue: JobQueue,
    crypto: CryptoService,
    github: Option<GitHubApp>,
    events: EventPublisher,
    email: Option<EmailService>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mcp_builder=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env()?;
    tracing::info!("Starting MCP Cloud Builder Worker");

    let db_pool = mcp_db::create_pool(&config).await?;
    let job_queue = JobQueue::connect(&config.redis.url).await?;

    // Create crypto service for decrypting secrets
    let crypto = CryptoService::from_hex(
        &std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set"),
    )
    .expect("Invalid encryption key");

    // Create GitHub App client (optional - may not have valid credentials in dev)
    let github = GitHubApp::new(&config).ok();
    if github.is_some() {
        tracing::info!("GitHub App initialized");
    } else {
        tracing::warn!("GitHub App not configured - will use public repos only");
    }

    // Create event publisher for real-time WebSocket updates
    let events = EventPublisher::new(&config.redis.url);

    // Create email service (optional)
    let email = match EmailService::from_env() {
        Ok(service) => {
            tracing::info!("Resend email service initialized");
            Some(service)
        }
        Err(e) => {
            tracing::warn!("Email service not configured: {} - email notifications disabled", e);
            None
        }
    };

    let context = Arc::new(BuilderContext {
        config: config.clone(),
        db: db_pool,
        job_queue,
        crypto,
        github,
        events,
        email,
    });

    // Connect to Redis for job queue
    let redis_url = &config.redis.url;
    let redis_client = redis::Client::open(redis_url.as_str())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;

    // Set poll interval to 30 seconds to reduce Redis commands
    // (default is too frequent and consumes Upstash free tier quickly)
    // Use explicit namespaces to match the API queue configuration
    let build_config = apalis_redis::Config::default()
        .set_namespace("build_jobs")
        .set_poll_interval(std::time::Duration::from_secs(30));
    let deploy_config = apalis_redis::Config::default()
        .set_namespace("deploy_jobs")
        .set_poll_interval(std::time::Duration::from_secs(30));

    // Log the key names that apalis-redis will use
    tracing::info!("Build queue config: namespace={:?}", build_config.get_namespace());
    tracing::info!("  active_jobs_list: {}", build_config.active_jobs_list());
    tracing::info!("  job_data_hash: {}", build_config.job_data_hash());

    let storage = RedisStorage::<BuildJob>::new_with_config(
        redis_conn.clone(),
        build_config,
    );
    let deploy_storage = RedisStorage::<DeployJob>::new_with_config(
        redis_conn,
        deploy_config,
    );

    tracing::info!("Connected to job queue, created BuildJob and DeployJob storage instances");

    // Create workers
    let build_worker = WorkerBuilder::new("build-worker")
        .data(context.clone())
        .backend(storage)
        .build_fn(handle_build_job);

    let deploy_worker = WorkerBuilder::new("deploy-worker")
        .data(context.clone())
        .backend(deploy_storage)
        .build_fn(handle_deploy_job);

    // Start health check HTTP server in background (required for Fly.io to keep machine running)
    let health_port = std::env::var("HEALTH_PORT").unwrap_or_else(|_| "8080".to_string());
    let health_addr = format!("0.0.0.0:{}", health_port);

    let health_router = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/", get(|| async { "MCP Cloud Builder Worker" }));

    let health_listener = TcpListener::bind(&health_addr).await?;
    tracing::info!("Health check server listening on {}", health_addr);

    // Spawn health server in background task
    tokio::spawn(async move {
        if let Err(e) = axum::serve(health_listener, health_router).await {
            tracing::error!("Health server error: {}", e);
        }
    });

    tracing::info!("Starting job queue workers (poll interval: 30s)...");

    // Spawn a heartbeat task that verifies Redis connectivity
    // If Redis connection fails, exit the process so Fly.io restarts it
    let redis_heartbeat_url = config.redis.url.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        let mut consecutive_failures = 0;
        const MAX_FAILURES: u32 = 3;
        // Scalability: Configurable timeout for Redis operations
        let redis_timeout_secs: u64 = std::env::var("REDIS_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(15);

        loop {
            interval.tick().await;

            // Try to ping Redis with timeout
            let ping_result = tokio::time::timeout(
                std::time::Duration::from_secs(redis_timeout_secs),
                async {
                    let client = redis::Client::open(redis_heartbeat_url.as_str())?;
                    let mut conn = client.get_multiplexed_async_connection().await?;
                    redis::cmd("PING").query_async::<String>(&mut conn).await
                }
            ).await;

            match ping_result {
                Ok(Ok(_)) => {
                    consecutive_failures = 0;
                    tracing::info!("Builder heartbeat - Redis connection OK");
                }
                Ok(Err(e)) => {
                    consecutive_failures += 1;
                    tracing::error!("Redis PING failed: {} ({}/{})", e, consecutive_failures, MAX_FAILURES);
                }
                Err(_) => {
                    consecutive_failures += 1;
                    tracing::error!("Redis PING timed out after {}s ({}/{})", redis_timeout_secs, consecutive_failures, MAX_FAILURES);
                }
            }

            if consecutive_failures >= MAX_FAILURES {
                tracing::error!("Redis connection failed {} times consecutively, exiting for restart", MAX_FAILURES);
                std::process::exit(1);
            }
        }
    });

    // Run workers - this blocks forever, polling for jobs
    let monitor = Monitor::new()
        .register(build_worker)
        .register(deploy_worker);

    tracing::info!("Monitor created with 2 workers, calling run()...");

    // Run the monitor - this should block and poll for jobs
    match monitor.run().await {
        Ok(()) => {
            tracing::info!("Monitor.run() completed successfully");
        }
        Err(e) => {
            tracing::error!("Monitor.run() failed with error: {:?}", e);
        }
    }

    tracing::warn!("Monitor exited - this should not happen in normal operation!");
    Ok(())
}

async fn handle_build_job(mut job: BuildJob, ctx: Data<Arc<BuilderContext>>) -> Result<(), Error> {
    tracing::info!("Processing build job: {:?}", job.deployment_id);

    // Update deployment status to building
    DeploymentRepository::update(
        &ctx.db,
        job.deployment_id,
        UpdateDeployment {
            status: Some(mcp_common::types::DeploymentStatus::Building),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| Error::Failed(Arc::new(e.into())))?;

    // Publish building status via WebSocket
    ctx.events
        .publish_deployment_status(
            job.deployment_id,
            job.server_id,
            mcp_common::types::DeploymentStatus::Building,
            None,
            Some(10),
        )
        .await
        .ok();

    // Parse owner/repo from github_repo
    let parts: Vec<&str> = job.github_repo.split('/').collect();
    if parts.len() != 2 {
        let err_msg = format!("Invalid github_repo format: {}", job.github_repo);
        DeploymentRepository::update(
            &ctx.db,
            job.deployment_id,
            UpdateDeployment {
                status: Some(mcp_common::types::DeploymentStatus::Failed),
                error_message: Some(err_msg.clone()),
                finished_at: Some(chrono::Utc::now()),
                ..Default::default()
            },
        )
        .await
        .ok();
        return Err(Error::Failed(Arc::new(anyhow::anyhow!(err_msg).into())));
    }
    let (owner, repo) = (parts[0], parts[1]);

    // Create temp directory for source code
    let temp_dir = tempfile::tempdir()
        .map_err(|e| Error::Failed(Arc::new(e.into())))?;
    let source_dir = temp_dir.path();

    // Helper to log and publish to WebSocket
    // Takes ownership of String to avoid double allocation
    let log_to_db_and_ws = |ctx: &BuilderContext, deployment_id: uuid::Uuid, msg: String| {
        let db = ctx.db.clone();
        let events = ctx.events.clone();
        async move {
            DeploymentRepository::append_log(&db, deployment_id, &msg).await.ok();
            events.publish_build_log(deployment_id, &msg, LogStream::Stdout).await.ok();
        }
    };

    // Download source code
    let download_result: Result<(), anyhow::Error> = async {
        if let (Some(github), Some(installation_id)) = (&ctx.github, job.github_installation_id) {
            log_to_db_and_ws(&ctx, job.deployment_id, "Downloading source from GitHub...".to_string()).await;

            match github.download_tarball(installation_id, owner, repo, &job.github_branch).await {
                Ok(tarball) => {
                    log_to_db_and_ws(&ctx, job.deployment_id, format!("Downloaded {} bytes, extracting...", tarball.len())).await;
                    // Extract tarball to source_dir
                    extract_tarball(&tarball, source_dir)?;
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!("GitHub App download failed, falling back to git clone: {}", e);
                    log_to_db_and_ws(&ctx, job.deployment_id, "Falling back to git clone...".to_string()).await;
                    clone_repo(&job.github_repo, &job.github_branch, source_dir).await
                }
            }
        } else {
            log_to_db_and_ws(&ctx, job.deployment_id, "Cloning public repository...".to_string()).await;
            clone_repo(&job.github_repo, &job.github_branch, source_dir).await
        }
    }.await;

    if let Err(e) = download_result {
        let err_msg = format!("Failed to download source: {}", e);
        handle_build_failure(&ctx, &job, &err_msg).await;
        return Err(Error::Failed(Arc::new(e.into())));
    }

    // Determine the actual source directory (respecting root_directory setting)
    let actual_source_dir = if job.root_directory.is_empty() || job.root_directory == "." || job.root_directory == "/" {
        source_dir.to_path_buf()
    } else {
        // SECURITY: Validate root_directory to prevent path traversal attacks
        // Block directory traversal sequences
        if job.root_directory.contains("..") {
            let err_msg = "Root directory cannot contain '..' (path traversal not allowed)";
            handle_build_failure(&ctx, &job, err_msg).await;
            return Err(Error::Failed(Arc::new(anyhow::anyhow!(err_msg).into())));
        }

        // Strip leading slash if present
        let clean_root_dir = job.root_directory.trim_start_matches('/');

        // Block absolute paths and other dangerous patterns
        if clean_root_dir.starts_with('/') || clean_root_dir.contains('\0') {
            let err_msg = "Invalid root directory path";
            handle_build_failure(&ctx, &job, err_msg).await;
            return Err(Error::Failed(Arc::new(anyhow::anyhow!(err_msg).into())));
        }

        let subdir = source_dir.join(clean_root_dir);

        // Verify the subdirectory exists
        if !subdir.exists() {
            let err_msg = format!("Root directory '{}' not found in repository", job.root_directory);
            handle_build_failure(&ctx, &job, &err_msg).await;
            return Err(Error::Failed(Arc::new(anyhow::anyhow!(err_msg).into())));
        }

        // SECURITY: Canonicalize and verify the path is within source_dir
        let canonical_subdir = match subdir.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                let err_msg = format!("Failed to resolve root directory: {}", e);
                handle_build_failure(&ctx, &job, &err_msg).await;
                return Err(Error::Failed(Arc::new(anyhow::anyhow!(err_msg).into())));
            }
        };

        let canonical_source = match source_dir.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                let err_msg = format!("Failed to resolve source directory: {}", e);
                handle_build_failure(&ctx, &job, &err_msg).await;
                return Err(Error::Failed(Arc::new(anyhow::anyhow!(err_msg).into())));
            }
        };

        // Ensure the resolved path is still within the source directory
        if !canonical_subdir.starts_with(&canonical_source) {
            let err_msg = "Root directory path escapes repository boundary (path traversal blocked)";
            handle_build_failure(&ctx, &job, err_msg).await;
            return Err(Error::Failed(Arc::new(anyhow::anyhow!(err_msg).into())));
        }

        log_to_db_and_ws(&ctx, job.deployment_id, format!("Using root directory: {}", job.root_directory)).await;
        canonical_subdir
    };

    // Strategy 3: a member of an npm/yarn workspaces monorepo cannot be built from
    // its own subdirectory — its tsconfig typically extends the repo root and its
    // dependencies are hoisted to the root node_modules. When the target is such a
    // member and the user did not pin an entry command, build from the repo ROOT and
    // point the entry at the member's output (e.g. `node src/filesystem/dist/index.js`),
    // detected from the member's own manifest. Otherwise keep the subdirectory context.
    let mut build_context = actual_source_dir.clone();
    if !job.root_directory.is_empty()
        && job.root_directory != "."
        && job.root_directory != "/"
        && job.entry_command.is_none()
    {
        let clean_root_dir = job.root_directory.trim_start_matches('/').to_string();
        let workspaces = flyctl::detect_workspace_globs(source_dir);
        if flyctl::subdir_is_workspace_member(&clean_root_dir, &workspaces) {
            if let Some(entry) =
                flyctl::detect_member_entry(source_dir, &actual_source_dir, &job.runtime).await
            {
                let prefixed = flyctl::prefix_entry_with_subdir(&entry, &clean_root_dir);
                log_to_db_and_ws(
                    &ctx,
                    job.deployment_id,
                    format!(
                        "Workspace monorepo member detected — building from repo root with entry `{}`",
                        prefixed
                    ),
                )
                .await;
                job.entry_command = Some(prefixed);
                build_context = source_dir.to_path_buf();
            }
        }
    }

    // Get secrets for this server and decrypt them
    let encrypted_secrets = SecretRepository::list_by_server(&ctx.db, job.server_id)
        .await
        .unwrap_or_default();

    let secrets: Vec<mcp_queue::SecretEnv> = encrypted_secrets
        .into_iter()
        .filter_map(|secret| {
            ctx.crypto
                .decrypt(&secret.encrypted_value, &secret.nonce)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .map(|value| mcp_queue::SecretEnv {
                    key: secret.key,
                    value,
                })
        })
        .collect();

    // Update to deploying status
    DeploymentRepository::update(
        &ctx.db,
        job.deployment_id,
        UpdateDeployment {
            status: Some(mcp_common::types::DeploymentStatus::Deploying),
            ..Default::default()
        },
    )
    .await
    .ok();

    ctx.events
        .publish_deployment_status(
            job.deployment_id,
            job.server_id,
            mcp_common::types::DeploymentStatus::Deploying,
            None,
            Some(50),
        )
        .await
        .ok();

    // Build and deploy using flyctl (remote builder)
    let db_clone = ctx.db.clone();
    let events_clone = ctx.events.clone();
    let deployment_id = job.deployment_id;

    let on_log = move |msg: &str| {
        let db = db_clone.clone();
        let events = events_clone.clone();
        let msg = msg.to_string();
        tokio::spawn(async move {
            DeploymentRepository::append_log(&db, deployment_id, &msg).await.ok();
            events.publish_build_log(deployment_id, &msg, LogStream::Stdout).await.ok();
        });
    };

    let build_result = flyctl::build_and_deploy(
        &ctx.config,
        &job,
        &build_context,
        &secrets,
        on_log.clone(),
    )
    .await;

    // Judgment D: a built image and a started machine don't prove the MCP server
    // runs — a wrong entry path leaves the adapter listening while the child process
    // crash-loops, so the deploy looks successful but every request 500s. Probe a
    // real MCP initialize and demote a proven-broken server to a failure so it goes
    // through normal failure reporting instead of being shown as "succeeded".
    let build_result = match build_result {
        Ok(deploy_result) => {
            match flyctl::verify_mcp_initialize(&deploy_result.endpoint_url, &job.mcp_path, &on_log).await {
                flyctl::ProbeOutcome::Verified => Ok(deploy_result),
                flyctl::ProbeOutcome::Inconclusive(detail) => {
                    on_log(&format!(
                        "Warning: could not verify the MCP server responded; leaving as deployed. ({})",
                        detail
                    ));
                    Ok(deploy_result)
                }
                flyctl::ProbeOutcome::Broken(detail) => {
                    // Surface the real reason: the adapter's 500 only says the child
                    // exited, so fetch the server's own logs and show the actual error
                    // (e.g. `Cannot find module '/app/dist/index.js'`) instead of a
                    // generic "check the server logs".
                    let server_logs = flyctl::fetch_app_logs(&ctx.config, &deploy_result.app_name)
                        .await
                        .map(|l| flyctl::extract_error_lines(&l, 12))
                        .filter(|s| !s.is_empty());
                    let message = match server_logs {
                        Some(logs) => format!(
                            "Deployment reached Fly.io but the MCP server did not start. \
                            This is usually a wrong startup command or missing build output.\n\n\
                            Server error:\n{}",
                            logs
                        ),
                        None => format!(
                            "Deployment reached Fly.io but the MCP server did not start \
                            ({}). This is usually a wrong startup command or missing build output.",
                            detail
                        ),
                    };
                    Err(anyhow::anyhow!(message))
                }
            }
        }
        Err(e) => Err(e),
    };

    match build_result {
        Ok(deploy_result) => {
            // Deployment successful
            DeploymentRepository::update(
                &ctx.db,
                job.deployment_id,
                UpdateDeployment {
                    status: Some(mcp_common::types::DeploymentStatus::Succeeded),
                    finished_at: Some(chrono::Utc::now()),
                    ..Default::default()
                },
            )
            .await
            .ok();

            ctx.events
                .publish_deployment_status(
                    job.deployment_id,
                    job.server_id,
                    mcp_common::types::DeploymentStatus::Succeeded,
                    None,
                    Some(100),
                )
                .await
                .ok();

            if let Err(e) = ServerRepository::update_status(
                &ctx.db,
                job.server_id,
                mcp_common::types::ServerStatus::Running,
                Some(&deploy_result.endpoint_url),
            )
            .await
            {
                tracing::error!("Failed to update server status to Running: {}", e);
            }

            // Update region status with machine_id
            if let Err(e) = ServerRegionRepository::update(
                &ctx.db,
                job.server_id,
                &job.region,
                UpdateServerRegion {
                    status: Some(RegionStatus::Running),
                    endpoint_url: Some(deploy_result.endpoint_url),
                    machine_id: deploy_result.machine_id,
                },
            )
            .await
            {
                tracing::error!("Failed to update region status: {}", e);
            }

            tracing::info!("Deployment {} succeeded", job.deployment_id);

            // Send success email notification
            send_deploy_notification(&ctx, job.server_id, true, None).await;
        }
        Err(e) => {
            tracing::error!("Build failed: {}", e);
            let error_msg = e.to_string();

            // Get user's locale preference and analyze error with localized hints
            let locale = get_user_locale_for_server(&ctx.db, job.server_id).await;
            let hint = analyze_error_for_hints(&ctx.db, &error_msg, &locale).await;
            let full_error_msg = if let Some(ref hint) = hint {
                format!("{}{}", error_msg, hint)
            } else {
                error_msg.clone()
            };

            DeploymentRepository::update(
                &ctx.db,
                job.deployment_id,
                UpdateDeployment {
                    status: Some(mcp_common::types::DeploymentStatus::Failed),
                    error_message: Some(full_error_msg.clone()),
                    finished_at: Some(chrono::Utc::now()),
                    ..Default::default()
                },
            )
            .await
            .ok();

            // Publish failed status
            ctx.events
                .publish_deployment_status(
                    job.deployment_id,
                    job.server_id,
                    mcp_common::types::DeploymentStatus::Failed,
                    Some(full_error_msg.clone()),
                    Some(100),
                )
                .await
                .ok();
            ctx.events.publish_build_log(job.deployment_id, &error_msg, LogStream::Stderr).await.ok();

            // Log hint separately if available
            if let Some(hint) = hint {
                ctx.events
                    .publish_build_log(job.deployment_id, &hint, LogStream::Stderr)
                    .await
                    .ok();
                DeploymentRepository::append_log(&ctx.db, job.deployment_id, &hint)
                    .await
                    .ok();
            }

            // Only set server to failed if there's no newer successful deployment
            // This prevents race conditions where an old failed deployment (due to retries)
            // overwrites a newer successful deployment's status
            let current_version = DeploymentRepository::find_by_id(&ctx.db, job.deployment_id)
                .await
                .ok()
                .flatten()
                .map(|d| d.version)
                .unwrap_or(0);

            let has_newer_success = DeploymentRepository::has_succeeded_deployment_after(
                &ctx.db,
                job.server_id,
                current_version,
            )
            .await
            .unwrap_or(false);

            if has_newer_success {
                tracing::info!(
                    "Skipping server status update to Failed for deployment {} (version {}) - newer successful deployment exists",
                    job.deployment_id,
                    current_version
                );
            } else {
                if let Err(e) = ServerRepository::update_status(
                    &ctx.db,
                    job.server_id,
                    mcp_common::types::ServerStatus::Failed,
                    None,
                )
                .await
                {
                    tracing::error!("Failed to update server status to Failed: {}", e);
                }
            }

            // Send failure email notification
            send_deploy_notification(&ctx, job.server_id, false, Some(&full_error_msg)).await;
        }
    }

    Ok(())
}

/// Helper to handle build failure
async fn handle_build_failure(ctx: &BuilderContext, job: &BuildJob, error_msg: &str) {
    // Get user's locale preference and analyze error with localized hints
    let locale = get_user_locale_for_server(&ctx.db, job.server_id).await;
    let hint = analyze_error_for_hints(&ctx.db, error_msg, &locale).await;
    let full_error_msg = if let Some(hint) = &hint {
        format!("{}{}", error_msg, hint)
    } else {
        error_msg.to_string()
    };

    DeploymentRepository::update(
        &ctx.db,
        job.deployment_id,
        UpdateDeployment {
            status: Some(mcp_common::types::DeploymentStatus::Failed),
            error_message: Some(full_error_msg.clone()),
            finished_at: Some(chrono::Utc::now()),
            ..Default::default()
        },
    )
    .await
    .ok();

    ctx.events
        .publish_deployment_status(
            job.deployment_id,
            job.server_id,
            mcp_common::types::DeploymentStatus::Failed,
            Some(full_error_msg.clone()),
            Some(100),
        )
        .await
        .ok();

    // Log error message
    ctx.events
        .publish_build_log(job.deployment_id, error_msg, LogStream::Stderr)
        .await
        .ok();

    // Log hint separately if available
    if let Some(hint) = hint {
        ctx.events
            .publish_build_log(job.deployment_id, &hint, LogStream::Stderr)
            .await
            .ok();

        // Also append hint to deployment log
        DeploymentRepository::append_log(&ctx.db, job.deployment_id, &hint)
            .await
            .ok();
    }

    // Only set server to failed if there's no newer successful deployment
    // This prevents race conditions where an old failed deployment overwrites a newer success
    let current_version = DeploymentRepository::find_by_id(&ctx.db, job.deployment_id)
        .await
        .ok()
        .flatten()
        .map(|d| d.version)
        .unwrap_or(0);

    let has_newer_success = DeploymentRepository::has_succeeded_deployment_after(
        &ctx.db,
        job.server_id,
        current_version,
    )
    .await
    .unwrap_or(false);

    if !has_newer_success {
        ServerRepository::update_status(
            &ctx.db,
            job.server_id,
            mcp_common::types::ServerStatus::Failed,
            None,
        )
        .await
        .ok();
    } else {
        tracing::info!(
            "Skipping server status update to Failed for deployment {} (version {}) - newer successful deployment exists",
            job.deployment_id,
            current_version
        );
    }

    send_deploy_notification(ctx, job.server_id, false, Some(&full_error_msg)).await;
}

/// Extract tarball to directory
fn extract_tarball(tarball: &[u8], dest: &std::path::Path) -> anyhow::Result<()> {
    use flate2::read::GzDecoder;
    use std::io::Cursor;
    use tar::Archive;

    let gz = GzDecoder::new(Cursor::new(tarball));
    let mut archive = Archive::new(gz);

    // GitHub tarballs have a top-level directory like "owner-repo-sha/"
    // We need to strip this prefix
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();

        // Skip symlinks for security
        if entry.header().entry_type().is_symlink() {
            continue;
        }

        // Strip the first component (the top-level directory)
        let components: Vec<_> = path.components().collect();
        if components.len() > 1 {
            let stripped: std::path::PathBuf = components[1..].iter().collect();
            let dest_path = dest.join(&stripped);

            if entry.header().entry_type().is_dir() {
                std::fs::create_dir_all(&dest_path)?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                entry.unpack(&dest_path)?;
            }
        }
    }

    Ok(())
}

/// Validate git branch name to prevent command injection
/// Branch names must follow git naming conventions
fn validate_branch_name(branch: &str) -> anyhow::Result<()> {
    if branch.is_empty() {
        anyhow::bail!("Branch name cannot be empty");
    }

    // Max length check
    if branch.len() > 255 {
        anyhow::bail!("Branch name exceeds maximum length of 255 characters");
    }

    // Cannot start with dash (could be interpreted as option)
    if branch.starts_with('-') {
        anyhow::bail!("Branch name cannot start with a dash");
    }

    // Must not contain dangerous characters
    let forbidden_chars = [' ', '\t', '\n', '\r', '\0', '~', '^', ':', '?', '*', '[', '\\'];
    for c in branch.chars() {
        if forbidden_chars.contains(&c) {
            anyhow::bail!("Branch name contains invalid character: {:?}", c);
        }
    }

    // Must not contain ..
    if branch.contains("..") {
        anyhow::bail!("Branch name cannot contain '..'");
    }

    // Must not end with .lock
    if branch.ends_with(".lock") {
        anyhow::bail!("Branch name cannot end with '.lock'");
    }

    // Must not be empty after trimming
    if branch.trim().is_empty() {
        anyhow::bail!("Branch name cannot be empty or whitespace only");
    }

    Ok(())
}

/// Validate GitHub repository name to prevent injection
fn validate_github_repo(repo: &str) -> anyhow::Result<()> {
    if repo.is_empty() {
        anyhow::bail!("Repository name cannot be empty");
    }

    // Must be in owner/repo format
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        anyhow::bail!("Repository must be in 'owner/repo' format");
    }

    let owner = parts[0];
    let repo_name = parts[1];

    // Validate owner
    if owner.is_empty() || owner.len() > 39 {
        anyhow::bail!("Invalid repository owner");
    }

    // Owner must start with alphanumeric
    if !owner.chars().next().unwrap().is_ascii_alphanumeric() {
        anyhow::bail!("Repository owner must start with alphanumeric character");
    }

    // Owner must contain only alphanumeric and hyphens
    if !owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        anyhow::bail!("Repository owner contains invalid characters");
    }

    // Validate repo name
    if repo_name.is_empty() || repo_name.len() > 100 {
        anyhow::bail!("Invalid repository name length");
    }

    // Repo name must contain only alphanumeric, hyphens, underscores, and dots
    if !repo_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
        anyhow::bail!("Repository name contains invalid characters");
    }

    Ok(())
}

/// Clone repository using git
async fn clone_repo(
    github_repo: &str,
    branch: &str,
    dest: &std::path::Path,
) -> anyhow::Result<()> {
    // Validate inputs to prevent command injection
    validate_github_repo(github_repo)?;
    validate_branch_name(branch)?;

    let output = tokio::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            branch,
            &format!("https://github.com/{}.git", github_repo),
            dest.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Combine stdout and stderr for error message
        let mut error_details = String::new();
        if !stdout.is_empty() {
            error_details.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !error_details.is_empty() {
                error_details.push('\n');
            }
            error_details.push_str(&stderr);
        }

        return Err(anyhow::anyhow!("Git clone failed: {}", error_details.trim()));
    }

    Ok(())
}

async fn handle_deploy_job(job: DeployJob, ctx: Data<Arc<BuilderContext>>) -> Result<(), Error> {
    tracing::info!("Processing deploy job: {:?} to region {}", job.deployment_id, job.region);

    // Update deployment status
    DeploymentRepository::update(
        &ctx.db,
        job.deployment_id,
        UpdateDeployment {
            status: Some(mcp_common::types::DeploymentStatus::Deploying),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| Error::Failed(Arc::new(e.into())))?;

    // Update region status to deploying
    ServerRegionRepository::update_status(
        &ctx.db,
        job.server_id,
        &job.region,
        RegionStatus::Deploying,
    )
    .await
    .ok();

    // Publish deploying status
    ctx.events
        .publish_deployment_status(
            job.deployment_id,
            job.server_id,
            mcp_common::types::DeploymentStatus::Deploying,
            None,
            Some(80),
        )
        .await
        .ok();

    // Deploy to Fly.io
    match flyio::deploy(&ctx.config, &job).await {
        Ok(endpoint_url) => {
            DeploymentRepository::update(
                &ctx.db,
                job.deployment_id,
                UpdateDeployment {
                    status: Some(mcp_common::types::DeploymentStatus::Succeeded),
                    finished_at: Some(chrono::Utc::now()),
                    ..Default::default()
                },
            )
            .await
            .ok();

            // Publish succeeded status
            ctx.events
                .publish_deployment_status(
                    job.deployment_id,
                    job.server_id,
                    mcp_common::types::DeploymentStatus::Succeeded,
                    None,
                    Some(100),
                )
                .await
                .ok();

            if let Err(e) = ServerRepository::update_status(
                &ctx.db,
                job.server_id,
                mcp_common::types::ServerStatus::Running,
                Some(&endpoint_url),
            )
            .await
            {
                tracing::error!("Failed to update server status to Running: {}", e);
            }

            // Update region status to running with endpoint URL
            if let Err(e) = ServerRegionRepository::update(
                &ctx.db,
                job.server_id,
                &job.region,
                UpdateServerRegion {
                    status: Some(RegionStatus::Running),
                    endpoint_url: Some(endpoint_url.clone()),
                    ..Default::default()
                },
            )
            .await
            {
                tracing::error!("Failed to update region status: {}", e);
            }

            tracing::info!(
                "Deploy to region {} succeeded for server {}",
                job.region,
                job.server_id
            );

            // Send success email notification
            send_deploy_notification(&ctx, job.server_id, true, None).await;
        }
        Err(e) => {
            tracing::error!("Deploy to region {} failed: {}", job.region, e);
            let error_msg = e.to_string();

            // Get user's locale preference and analyze error with localized hints
            let locale = get_user_locale_for_server(&ctx.db, job.server_id).await;
            let hint = analyze_error_for_hints(&ctx.db, &error_msg, &locale).await;
            let full_error_msg = if let Some(ref hint) = hint {
                format!("{}{}", error_msg, hint)
            } else {
                error_msg.clone()
            };

            DeploymentRepository::update(
                &ctx.db,
                job.deployment_id,
                UpdateDeployment {
                    status: Some(mcp_common::types::DeploymentStatus::Failed),
                    error_message: Some(full_error_msg.clone()),
                    finished_at: Some(chrono::Utc::now()),
                    ..Default::default()
                },
            )
            .await
            .ok();

            // Publish failed status
            ctx.events
                .publish_deployment_status(
                    job.deployment_id,
                    job.server_id,
                    mcp_common::types::DeploymentStatus::Failed,
                    Some(full_error_msg.clone()),
                    Some(100),
                )
                .await
                .ok();

            // Only set server to failed if this is the latest deployment
            let should_set_failed = match DeploymentRepository::find_latest_by_server(&ctx.db, job.server_id).await {
                Ok(Some(latest)) => latest.id == job.deployment_id,
                _ => true,
            };

            if should_set_failed {
                if let Err(e) = ServerRepository::update_status(
                    &ctx.db,
                    job.server_id,
                    mcp_common::types::ServerStatus::Failed,
                    None,
                )
                .await
                {
                    tracing::error!("Failed to update server status to Failed: {}", e);
                }
            } else {
                tracing::info!(
                    "Skipping server status update to Failed for deployment {} - newer deployment exists",
                    job.deployment_id
                );
            }

            // Update region status to failed
            ServerRegionRepository::update_status(
                &ctx.db,
                job.server_id,
                &job.region,
                RegionStatus::Failed,
            )
            .await
            .ok();

            // Send failure email notification
            send_deploy_notification(&ctx, job.server_id, false, Some(&full_error_msg)).await;
        }
    }

    Ok(())
}
