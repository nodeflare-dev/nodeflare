//! Session affinity for stateful MCP servers running on more than one Fly Machine.
//!
//! Streamable HTTP MCP servers keep their session state (keyed by `Mcp-Session-Id`)
//! in the memory of whichever Machine served `initialize`. Fly's edge load-balances
//! across Machines, so a follow-up request can land on a *different* Machine that has
//! never seen the session and replies `400 Bad Request: Invalid session ID`.
//!
//! We pin each session to its owning Machine using Fly's `fly-force-instance-id`
//! request header:
//!   - `initialize` (no incoming session): pick a (preferably started) Machine, force
//!     it, then record `session-id -> machine-id` once the Machine returns the new
//!     session id in its response.
//!   - follow-up (carries `Mcp-Session-Id`): look the session up and force the same
//!     Machine.
//!   - anything else (stateless calls, legacy SSE, metadata, browsers): no header is
//!     added, so routing is unchanged — no behavioural regression.
//!
//! Everything here is best-effort: any failure (no Fly token, API error, unknown
//! session, non-`.fly.dev` endpoint, single-Machine app) yields *no* forced header,
//! i.e. exactly today's round-robin behaviour. There is deliberately no fallback
//! branch for "the pinned Machine is stopped" — Fly's proxy wakes a stopped Machine
//! when a request is forced to it (verified empirically), so forcing always resolves.

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::http::HeaderMap;
use fred::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ProxyState;

/// How long a `session-id -> machine-id` binding lives in Redis (refreshed on use).
const SESSION_TTL_SECS: i64 = 3600;
/// How long the per-app Machine list is cached in Redis.
const MACHINE_LIST_TTL_SECS: i64 = 30;

/// Round-robin cursor for picking a Machine on `initialize`.
static RR_CURSOR: AtomicUsize = AtomicUsize::new(0);

/// Outcome of an affinity decision for a single request.
#[derive(Default)]
pub struct Affinity {
    /// Machine id to pin this request to via `fly-force-instance-id`, if any.
    pub forced_machine: Option<String>,
    /// When set, this request is an `initialize`: bind the session id returned by the
    /// upstream to this Machine id once the response is available.
    pub bind_session_to: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Machine {
    id: String,
    state: String,
}

#[derive(Deserialize)]
struct FlyMachine {
    id: String,
    state: String,
}

/// Decide whether (and where) to pin this request.
pub async fn decide(
    state: &ProxyState,
    target_url: &str,
    headers: &HeaderMap,
    is_initialize: bool,
) -> Affinity {
    // Follow-up request: route to the Machine that owns this session.
    if let Some(session_id) = headers.get("mcp-session-id").and_then(|v| v.to_str().ok()) {
        if let Some(machine) = lookup_session(state, session_id).await {
            return Affinity {
                forced_machine: Some(machine),
                bind_session_to: None,
            };
        }
        // Unknown/expired session -> don't force; let Fly route normally.
        return Affinity::default();
    }

    // initialize: pick a Machine, force it, and bind the new session to it.
    if is_initialize {
        if let Some(app) = app_name_from_target(target_url) {
            let machines = machine_list(state, &app).await;
            if let Some(machine) = pick_machine(&machines) {
                return Affinity {
                    forced_machine: Some(machine.clone()),
                    bind_session_to: Some(machine),
                };
            }
        }
    }

    Affinity::default()
}

/// After an `initialize` response, persist `session-id -> machine-id`.
///
/// `session_id` is the `Mcp-Session-Id` the upstream returned (if any).
pub async fn capture_session(state: &ProxyState, affinity: &Affinity, session_id: Option<&str>) {
    if let (Some(machine), Some(session_id)) = (affinity.bind_session_to.as_deref(), session_id) {
        store_session(state, session_id, machine).await;
        tracing::info!("affinity: bound session {} -> machine {}", session_id, machine);
    }
}

/// `https://mcp-xxxx.fly.dev/mcp?...` -> `Some("mcp-xxxx")`. `None` for non-fly.dev hosts.
fn app_name_from_target(target_url: &str) -> Option<String> {
    let after_scheme = target_url.split("://").nth(1)?;
    let host = after_scheme.split('/').next()?;
    let host = host.split(':').next().unwrap_or(host);
    host.strip_suffix(".fly.dev").map(|s| s.to_string())
}

fn pick_machine(machines: &[Machine]) -> Option<String> {
    if machines.is_empty() {
        return None;
    }
    // Prefer already-running Machines so `initialize` doesn't pay a cold start; fall
    // back to any Machine (forcing wakes a stopped one).
    let started: Vec<&Machine> = machines.iter().filter(|m| m.state == "started").collect();
    let pool: Vec<&Machine> = if started.is_empty() {
        machines.iter().collect()
    } else {
        started
    };
    let idx = RR_CURSOR.fetch_add(1, Ordering::Relaxed) % pool.len();
    Some(pool[idx].id.clone())
}

async fn lookup_session(state: &ProxyState, session_id: &str) -> Option<String> {
    let key = session_key(session_id);
    let machine: Option<String> = state.redis.get(&key).await.ok().flatten();
    let machine = machine?;
    // Refresh the TTL on use (re-set; avoids version-specific EXPIRE signatures).
    let _: Result<(), _> = state
        .redis
        .set(
            &key,
            machine.as_str(),
            Some(Expiration::EX(SESSION_TTL_SECS)),
            None,
            false,
        )
        .await;
    Some(machine)
}

async fn store_session(state: &ProxyState, session_id: &str, machine: &str) {
    let key = session_key(session_id);
    let _: Result<(), _> = state
        .redis
        .set(
            &key,
            machine,
            Some(Expiration::EX(SESSION_TTL_SECS)),
            None,
            false,
        )
        .await;
}

fn session_key(session_id: &str) -> String {
    format!("proxy:mcpaff:sess:{}", session_id)
}

/// Fetch the app's Machine list, cached in Redis for `MACHINE_LIST_TTL_SECS`.
async fn machine_list(state: &ProxyState, app: &str) -> Vec<Machine> {
    let cache_key = format!("proxy:mcpaff:machines:{}", app);

    let cached: Option<String> = state.redis.get(&cache_key).await.ok().flatten();
    if let Some(json) = cached {
        if let Ok(list) = serde_json::from_str::<Vec<Machine>>(&json) {
            return list;
        }
    }

    let machines = fetch_machines(state, app).await;
    if !machines.is_empty() {
        if let Ok(json) = serde_json::to_string(&machines) {
            let _: Result<(), _> = state
                .redis
                .set(
                    &cache_key,
                    json,
                    Some(Expiration::EX(MACHINE_LIST_TTL_SECS)),
                    None,
                    false,
                )
                .await;
        }
    }
    machines
}

async fn fetch_machines(state: &ProxyState, app: &str) -> Vec<Machine> {
    let token = &state.config.flyio.api_token;
    if token.is_empty() {
        return Vec::new();
    }

    let url = format!("https://api.machines.dev/v1/apps/{}/machines", app);
    let resp = state.http_client.get(&url).bearer_auth(token).send().await;

    let resp = match resp {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            tracing::warn!(
                "affinity: Fly machines API returned {} for app {}",
                r.status(),
                app
            );
            return Vec::new();
        }
        Err(e) => {
            tracing::warn!("affinity: Fly machines API error for app {}: {}", app, e);
            return Vec::new();
        }
    };

    match resp.json::<Vec<FlyMachine>>().await {
        Ok(list) => list
            .into_iter()
            .map(|m| Machine {
                id: m.id,
                state: m.state,
            })
            .collect(),
        Err(e) => {
            tracing::warn!("affinity: failed to parse Fly machines for app {}: {}", app, e);
            Vec::new()
        }
    }
}
