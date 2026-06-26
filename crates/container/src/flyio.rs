use crate::{Container, ContainerConfig, ContainerRuntime, ContainerStatus};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

const FLY_API_URL: &str = "https://api.machines.dev/v1";
const FLY_GRAPHQL_URL: &str = "https://api.fly.io/graphql";

/// SECURITY: Log detailed error server-side but return sanitized message
fn log_and_sanitize_error(operation: &str, status: reqwest::StatusCode, body: &str) -> String {
    tracing::error!("Fly.io {} failed: {} - {}", operation, status, body);
    format!("{} failed (status: {})", operation, status)
}

pub struct FlyioRuntime {
    api_token: String,
    org_slug: String,
    region: String,
    http_client: reqwest::Client,
}

impl FlyioRuntime {
    pub fn new(api_token: String, org_slug: String, region: String) -> Result<Self> {
        // SECURITY: Configure HTTP client with timeout and redirect policy
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            api_token,
            org_slug,
            region,
            http_client,
        })
    }

    fn app_name_from_container_name(&self, name: &str) -> String {
        name.replace("_", "-").to_lowercase()
    }

    /// Destroy an entire Fly.io app (including all machines)
    /// This is used when a server is deleted from nodeflare
    pub async fn destroy_app(&self, app_name: &str) -> Result<()> {
        let response = self
            .http_client
            .delete(format!("{}/apps/{}", FLY_API_URL, app_name))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .context("Failed to destroy app")?;

        // 404 is OK - app doesn't exist (already deleted or never created)
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            tracing::info!("Fly.io app {} not found (already deleted)", app_name);
            return Ok(());
        }

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("Destroy app", status, &error));
        }

        tracing::info!("Fly.io app {} destroyed successfully", app_name);
        Ok(())
    }

    /// Provisioned memory (MB) of every *started* machine in an app. Used by the usage
    /// sampler: billing follows actual running machines, so it naturally captures HA
    /// replicas (2 started machines = 2x) and any detection-driven memory bump, and bills
    /// nothing while auto-stop has the app idle. A missing app yields an empty list.
    pub async fn list_started_machine_memory_mb(&self, app_name: &str) -> Result<Vec<u32>> {
        let response = self
            .http_client
            .get(format!("{}/apps/{}/machines", FLY_API_URL, app_name))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .context("Failed to list machines")?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("List machines", status, &error));
        }

        let machines: Vec<MachineListItem> =
            response.json().await.context("Failed to parse machines list")?;
        Ok(machines
            .into_iter()
            .filter(|m| m.state == "started")
            .map(|m| m.config.guest.memory_mb)
            .collect())
    }

    /// Encode app_name and machine_id into a single ID string
    fn encode_id(app_name: &str, machine_id: &str) -> String {
        format!("{}:{}", app_name, machine_id)
    }

    /// Decode app_name and machine_id from an encoded ID string
    fn decode_id(id: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = id.splitn(2, ':').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid container ID format: {}", id);
        }
        Ok((parts[0].to_string(), parts[1].to_string()))
    }
}

#[derive(Debug, Serialize)]
struct CreateMachineRequest {
    name: String,
    region: String,
    config: MachineConfig,
}

#[derive(Debug, Serialize)]
struct MachineConfig {
    image: String,
    env: std::collections::HashMap<String, String>,
    guest: MachineGuest,
    services: Vec<MachineService>,
}

#[derive(Debug, Serialize)]
struct MachineGuest {
    cpu_kind: String,
    cpus: u8,
    memory_mb: u32,
}

#[derive(Debug, Serialize)]
struct MachineService {
    ports: Vec<MachinePort>,
    protocol: String,
    internal_port: u16,
}

#[derive(Debug, Serialize)]
struct MachinePort {
    port: u16,
    handlers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MachineResponse {
    id: String,
    #[allow(dead_code)]
    name: String,
    state: String,
}

/// Subset of the Fly machine object we need to bill running time (state + memory).
#[derive(Debug, Deserialize)]
struct MachineListItem {
    state: String,
    #[serde(default)]
    config: MachineListConfig,
}

#[derive(Debug, Deserialize, Default)]
struct MachineListConfig {
    #[serde(default)]
    guest: MachineListGuest,
}

#[derive(Debug, Deserialize, Default)]
struct MachineListGuest {
    #[serde(default)]
    memory_mb: u32,
}

#[async_trait::async_trait]
impl ContainerRuntime for FlyioRuntime {
    async fn create(&self, name: &str, config: ContainerConfig) -> Result<Container> {
        let app_name = self.app_name_from_container_name(name);

        // Ensure app exists
        let _ = self
            .http_client
            .post(format!("{}/apps", FLY_API_URL))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&serde_json::json!({
                "app_name": app_name,
                "org_slug": self.org_slug
            }))
            .send()
            .await;

        let request = CreateMachineRequest {
            name: name.to_string(),
            region: self.region.clone(),
            config: MachineConfig {
                image: config.image,
                env: config.env,
                guest: MachineGuest {
                    cpu_kind: "shared".to_string(),
                    cpus: 1,
                    memory_mb: config.memory_mb,
                },
                services: vec![MachineService {
                    ports: vec![
                        MachinePort {
                            port: 80,
                            handlers: vec!["http".to_string()],
                        },
                        MachinePort {
                            port: 443,
                            handlers: vec!["http".to_string(), "tls".to_string()],
                        },
                    ],
                    protocol: "tcp".to_string(),
                    internal_port: config.port,
                }],
            },
        };

        let response = self
            .http_client
            .post(format!("{}/apps/{}/machines", FLY_API_URL, app_name))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&request)
            .send()
            .await
            .context("Failed to create machine")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("Create machine", status, &error));
        }

        let machine: MachineResponse = response.json().await?;

        Ok(Container {
            id: Self::encode_id(&app_name, &machine.id),
            name: app_name.clone(),
            status: ContainerStatus::Creating,
            endpoint_url: Some(format!("https://{}.fly.dev", app_name)),
        })
    }

    async fn start(&self, id: &str) -> Result<()> {
        let (app_name, machine_id) = Self::decode_id(id)?;

        let response = self
            .http_client
            .post(format!(
                "{}/apps/{}/machines/{}/start",
                FLY_API_URL, app_name, machine_id
            ))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .context("Failed to start machine")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("Start machine", status, &error));
        }

        Ok(())
    }

    async fn stop(&self, id: &str) -> Result<()> {
        let (app_name, machine_id) = Self::decode_id(id)?;

        let response = self
            .http_client
            .post(format!(
                "{}/apps/{}/machines/{}/stop",
                FLY_API_URL, app_name, machine_id
            ))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .context("Failed to stop machine")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("Stop machine", status, &error));
        }

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let (app_name, machine_id) = Self::decode_id(id)?;

        let response = self
            .http_client
            .delete(format!(
                "{}/apps/{}/machines/{}",
                FLY_API_URL, app_name, machine_id
            ))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .context("Failed to delete machine")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("Delete machine", status, &error));
        }

        Ok(())
    }

    async fn status(&self, id: &str) -> Result<ContainerStatus> {
        let (app_name, machine_id) = Self::decode_id(id)?;

        let response = self
            .http_client
            .get(format!(
                "{}/apps/{}/machines/{}",
                FLY_API_URL, app_name, machine_id
            ))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .send()
            .await
            .context("Failed to get machine status")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("Get machine status", status, &error));
        }

        let machine: MachineResponse = response.json().await?;

        let status = match machine.state.as_str() {
            "started" | "running" => ContainerStatus::Running,
            "stopped" | "stopping" => ContainerStatus::Stopped,
            "created" | "starting" => ContainerStatus::Creating,
            _ => ContainerStatus::Failed,
        };

        Ok(status)
    }

    async fn logs(&self, id: &str, tail: usize) -> Result<String> {
        let (app_name, _machine_id) = Self::decode_id(id)?;

        // Fly.io logs are accessed via Nats or the logs API
        // For simplicity, we'll use the HTTP logs endpoint
        let response = self
            .http_client
            .get(format!("https://api.fly.io/api/v1/apps/{}/logs", app_name))
            .header("Authorization", format!("Bearer {}", self.api_token))
            .query(&[("limit", tail.to_string())])
            .send()
            .await
            .context("Failed to get logs")?;

        if !response.status().is_success() {
            // Logs endpoint may not be available, return empty
            return Ok(String::new());
        }

        let logs = response.text().await.unwrap_or_default();
        Ok(logs)
    }
}

// ============================================================================
// Extended Fly.io Features: Exec, WireGuard, Tigris
// ============================================================================

/// Response from machine exec
#[derive(Debug, Deserialize)]
pub struct ExecResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// WireGuard peer info (for listing)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardPeerInfo {
    pub name: String,
    pub region: String,
    pub peerip: String,
}

/// WireGuard configuration for client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireGuardConfig {
    pub peer_name: String,
    pub private_key: String,
    pub public_key: String,
    pub peer_ip: String,
    pub dns: String,
    pub endpoint: String,
    pub endpoint_public_key: String,
    pub allowed_ips: String,
}

impl FlyioRuntime {
    /// Execute a command on a running machine
    pub async fn exec(&self, id: &str, command: Vec<String>, timeout_secs: u32) -> Result<ExecResponse> {
        let (app_name, machine_id) = Self::decode_id(id)?;

        // Fly.io exec API expects cmd as a string, not an array
        let cmd_str = command.join(" ");

        let request = serde_json::json!({
            "cmd": cmd_str,
            "timeout": timeout_secs
        });

        let url = format!(
            "{}/apps/{}/machines/{}/exec",
            FLY_API_URL, app_name, machine_id
        );
        tracing::debug!("Executing command on {}: {:?}", url, command);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&request)
            .send()
            .await
            .context("Failed to execute command")?;

        let status = response.status();
        tracing::debug!("Exec response status: {}", status);

        if !status.is_success() {
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("Execute command", status, &error));
        }

        let body = response.text().await.context("Failed to read response body")?;
        tracing::debug!("Exec response body: {}", body);

        let result: ExecResponse = serde_json::from_str(&body)
            .context("Failed to parse exec response")?;
        Ok(result)
    }

    /// Create a WireGuard peer for an organization
    pub async fn create_wireguard_peer(
        &self,
        org_slug: &str,
        region: &str,
        peer_name: &str,
    ) -> Result<WireGuardConfig> {
        // Generate WireGuard keypair
        let private_key = Self::generate_wireguard_private_key();
        let public_key = Self::derive_wireguard_public_key(&private_key)?;

        let query = r#"
            mutation AddWireGuardPeer($input: AddWireGuardPeerInput!) {
                addWireGuardPeer(input: $input) {
                    peerip
                    endpointip
                    pubkey
                }
            }
        "#;

        let variables = serde_json::json!({
            "input": {
                "organizationId": org_slug,
                "region": region,
                "name": peer_name,
                "pubkey": public_key
            }
        });

        let response = self
            .http_client
            .post(FLY_GRAPHQL_URL)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&serde_json::json!({
                "query": query,
                "variables": variables
            }))
            .send()
            .await
            .context("Failed to create WireGuard peer")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("Create WireGuard peer", status, &error));
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(errors) = result.get("errors") {
            // SECURITY: Log full GraphQL error server-side only
            tracing::error!("GraphQL error creating WireGuard peer: {}", errors);
            anyhow::bail!("GraphQL operation failed");
        }

        let data = result
            .get("data")
            .and_then(|d| d.get("addWireGuardPeer"))
            .ok_or_else(|| anyhow!("Invalid response from WireGuard API"))?;

        let peer_ip = data["peerip"].as_str().unwrap_or("").to_string();
        let endpoint_ip = data["endpointip"].as_str().unwrap_or("").to_string();
        let endpoint_pubkey = data["pubkey"].as_str().unwrap_or("").to_string();

        Ok(WireGuardConfig {
            peer_name: peer_name.to_string(),
            private_key,
            public_key,
            peer_ip,
            dns: "fdaa::3".to_string(),
            endpoint: format!("{}:51820", endpoint_ip),
            endpoint_public_key: endpoint_pubkey,
            allowed_ips: "fdaa::/16".to_string(),
        })
    }

    /// Remove a WireGuard peer
    pub async fn remove_wireguard_peer(&self, org_slug: &str, peer_name: &str) -> Result<()> {
        let query = r#"
            mutation RemoveWireGuardPeer($input: RemoveWireGuardPeerInput!) {
                removeWireGuardPeer(input: $input) {
                    organization {
                        id
                    }
                }
            }
        "#;

        let variables = serde_json::json!({
            "input": {
                "organizationId": org_slug,
                "name": peer_name
            }
        });

        let response = self
            .http_client
            .post(FLY_GRAPHQL_URL)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&serde_json::json!({
                "query": query,
                "variables": variables
            }))
            .send()
            .await
            .context("Failed to remove WireGuard peer")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("Remove WireGuard peer", status, &error));
        }

        Ok(())
    }

    /// List WireGuard peers for an organization
    pub async fn list_wireguard_peers(&self, org_slug: &str) -> Result<Vec<WireGuardPeerInfo>> {
        let query = r#"
            query GetWireGuardPeers($slug: String!) {
                organization(slug: $slug) {
                    wireGuardPeers {
                        nodes {
                            name
                            region
                            peerip
                        }
                    }
                }
            }
        "#;

        let variables = serde_json::json!({
            "slug": org_slug
        });

        let response = self
            .http_client
            .post(FLY_GRAPHQL_URL)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&serde_json::json!({
                "query": query,
                "variables": variables
            }))
            .send()
            .await
            .context("Failed to list WireGuard peers")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            anyhow::bail!("{}", log_and_sanitize_error("List WireGuard peers", status, &error));
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(errors) = result.get("errors") {
            // SECURITY: Log full GraphQL error server-side only
            tracing::error!("GraphQL error listing WireGuard peers: {}", errors);
            anyhow::bail!("GraphQL operation failed");
        }

        let nodes = result
            .get("data")
            .and_then(|d| d.get("organization"))
            .and_then(|o| o.get("wireGuardPeers"))
            .and_then(|w| w.get("nodes"))
            .and_then(|n| n.as_array())
            .cloned()
            .unwrap_or_default();

        let peers: Vec<WireGuardPeerInfo> = nodes
            .into_iter()
            .filter_map(|node| {
                Some(WireGuardPeerInfo {
                    name: node.get("name")?.as_str()?.to_string(),
                    region: node.get("region")?.as_str()?.to_string(),
                    peerip: node.get("peerip")?.as_str()?.to_string(),
                })
            })
            .collect();

        Ok(peers)
    }

    /// Generate WireGuard configuration file content
    pub fn generate_wireguard_config(config: &WireGuardConfig) -> String {
        format!(
            r#"[Interface]
PrivateKey = {}
Address = {}/120
DNS = {}

[Peer]
PublicKey = {}
AllowedIPs = {}
Endpoint = {}
PersistentKeepalive = 15
"#,
            config.private_key,
            config.peer_ip,
            config.dns,
            config.endpoint_public_key,
            config.allowed_ips,
            config.endpoint
        )
    }

    // Helper: Generate WireGuard private key (base64 encoded)
    // SECURITY: Uses cryptographically secure RNG for key generation
    fn generate_wireguard_private_key() -> String {
        use ring::rand::{SecureRandom, SystemRandom};
        let rng = SystemRandom::new();
        let mut key = [0u8; 32];
        rng.fill(&mut key).expect("SystemRandom failed");
        // Clamp the key for Curve25519
        key[0] &= 248;
        key[31] &= 127;
        key[31] |= 64;
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, key)
    }

    // Helper: Derive public key from private key
    fn derive_wireguard_public_key(private_key: &str) -> Result<String> {
        use base64::Engine;
        let private_bytes = base64::engine::general_purpose::STANDARD
            .decode(private_key)
            .context("Invalid private key")?;

        if private_bytes.len() != 32 {
            anyhow::bail!("Invalid private key length");
        }

        // Use x25519-dalek for proper key derivation
        let mut private_array = [0u8; 32];
        private_array.copy_from_slice(&private_bytes);

        // Compute public key using scalar multiplication
        let public_key = x25519_dalek::x25519(private_array, x25519_dalek::X25519_BASEPOINT_BYTES);

        Ok(base64::engine::general_purpose::STANDARD.encode(public_key))
    }

    /// Get metrics for a specific app from Fly.io Prometheus API
    pub async fn get_metrics(&self, app_name: &str) -> Result<AppMetrics> {
        let now = chrono::Utc::now().timestamp();
        // Last 24h, not 1h: servers scale to zero, so the machine is usually suspended and
        // only produces metrics while it briefly runs. A wide window lets us surface the
        // last-seen sample (the UI labels it "as of <time>") instead of an empty 0.
        let start = now - 86_400;

        // Query memory metrics
        let memory_query = format!(
            "fly_instance_memory_mem_total{{app=\"{}\"}} - fly_instance_memory_mem_available{{app=\"{}\"}}",
            app_name, app_name
        );
        let memory_used = self.query_prometheus(&memory_query, start, now, "5m").await?;

        // Query memory total
        let memory_total_query = format!("fly_instance_memory_mem_total{{app=\"{}\"}}", app_name);
        let memory_total = self.query_prometheus(&memory_total_query, start, now, "5m").await?;

        // Query CPU usage (user + system time)
        let cpu_query = format!(
            "rate(fly_instance_cpu{{app=\"{}\",mode=~\"user|system\"}}[5m])",
            app_name
        );
        let cpu_usage = self.query_prometheus(&cpu_query, start, now, "5m").await?;

        // Query network rx/tx
        let network_rx_query = format!("rate(fly_instance_net_recv_bytes{{app=\"{}\"}}[5m])", app_name);
        let network_tx_query = format!("rate(fly_instance_net_sent_bytes{{app=\"{}\"}}[5m])", app_name);
        let network_rx = self.query_prometheus(&network_rx_query, start, now, "5m").await?;
        let network_tx = self.query_prometheus(&network_tx_query, start, now, "5m").await?;

        Ok(AppMetrics {
            memory_used,
            memory_total,
            cpu_usage,
            network_rx,
            network_tx,
        })
    }

    /// Query Prometheus API
    async fn query_prometheus(
        &self,
        query: &str,
        start: i64,
        end: i64,
        step: &str,
    ) -> Result<Vec<MetricDataPoint>> {
        let url = format!(
            "https://api.fly.io/prometheus/{}/api/v1/query_range",
            self.org_slug
        );

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .query(&[
                ("query", query),
                ("start", &start.to_string()),
                ("end", &end.to_string()),
                ("step", step),
            ])
            .send()
            .await
            .context("Failed to query Prometheus")?;

        if !response.status().is_success() {
            let status = response.status();
            let error = response.text().await.unwrap_or_default();
            tracing::warn!("Prometheus query failed: {} - {}", status, error);
            // Don't swallow into an empty vec: a 401/403 (token lacks Prometheus scope) or a
            // wrong org slug would otherwise be indistinguishable from "no data", surfacing as
            // a misleading all-zero dashboard. Propagate so the cause is visible.
            return Err(anyhow!(
                "Prometheus query failed (status {}): {}",
                status,
                error.trim()
            ));
        }

        let result: PrometheusResponse = response.json().await
            .context("Failed to parse Prometheus response")?;

        // Extract data points from the response
        let mut data_points = Vec::new();
        if let Some(data) = result.data {
            for series in data.result {
                for (timestamp, value) in series.values {
                    let val: f64 = value.parse().unwrap_or(0.0);
                    data_points.push(MetricDataPoint {
                        timestamp: timestamp as i64,
                        value: val,
                        instance: series.metric.get("instance").cloned(),
                    });
                }
            }
        }

        Ok(data_points)
    }
}

/// App metrics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMetrics {
    pub memory_used: Vec<MetricDataPoint>,
    pub memory_total: Vec<MetricDataPoint>,
    pub cpu_usage: Vec<MetricDataPoint>,
    pub network_rx: Vec<MetricDataPoint>,
    pub network_tx: Vec<MetricDataPoint>,
}

/// Single metric data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDataPoint {
    pub timestamp: i64,
    pub value: f64,
    pub instance: Option<String>,
}

/// Prometheus API response
#[derive(Debug, Deserialize)]
struct PrometheusResponse {
    #[allow(dead_code)]
    status: String,
    data: Option<PrometheusData>,
}

#[derive(Debug, Deserialize)]
struct PrometheusData {
    #[serde(rename = "resultType")]
    #[allow(dead_code)]
    result_type: String,
    result: Vec<PrometheusResult>,
}

#[derive(Debug, Deserialize)]
struct PrometheusResult {
    metric: std::collections::HashMap<String, String>,
    values: Vec<(f64, String)>,
}
