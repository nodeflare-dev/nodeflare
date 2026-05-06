use anyhow::{Context, Result};
use mcp_common::AppConfig;
use mcp_queue::DeployJob;
use serde::{Deserialize, Serialize};

const FLY_API_URL: &str = "https://api.machines.dev/v1";

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
    services: Vec<MachineService>,
    guest: MachineGuest,
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

#[derive(Debug, Serialize)]
struct MachineGuest {
    cpu_kind: String,
    cpus: u8,
    memory_mb: u32,
}

#[derive(Debug, Deserialize)]
struct MachineResponse {
    id: String,
    #[allow(dead_code)]
    name: String,
    state: String,
    #[allow(dead_code)]
    #[serde(default)]
    private_ip: Option<String>,
}

pub async fn deploy(config: &AppConfig, job: &DeployJob) -> Result<String> {
    let client = reqwest::Client::new();
    let server_id_str = job.server_id.to_string();
    let app_name = format!("mcp-{}", server_id_str.split('-').next().unwrap_or(&server_id_str[..8.min(server_id_str.len())]));

    // Create app if it doesn't exist
    create_app_if_not_exists(&client, config, &app_name).await?;

    // Build environment variables
    let mut env = std::collections::HashMap::new();
    for secret in &job.secrets {
        env.insert(secret.key.clone(), secret.value.clone());
    }
    env.insert("PORT".to_string(), "3000".to_string());

    // Create machine
    let request = CreateMachineRequest {
        name: format!("{}-machine", app_name),
        region: job.region.clone(),
        config: MachineConfig {
            image: job.image_url.clone(),
            env,
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
                internal_port: 3000,
            }],
            guest: MachineGuest {
                cpu_kind: "shared".to_string(),
                cpus: 1,
                memory_mb: 256,
            },
        },
    };

    let response = client
        .post(format!("{}/apps/{}/machines", FLY_API_URL, app_name))
        .header("Authorization", format!("Bearer {}", config.flyio.api_token))
        .json(&request)
        .send()
        .await
        .context("Failed to create machine")?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Failed to create machine: {}", error_text));
    }

    let machine: MachineResponse = response.json().await?;
    tracing::info!("Created machine: {} ({})", machine.name, machine.id);

    // Wait for machine to start
    wait_for_machine(&client, config, &app_name, &machine.id).await?;

    // Return the endpoint URL
    Ok(format!("https://{}.fly.dev", app_name))
}

async fn create_app_if_not_exists(
    client: &reqwest::Client,
    config: &AppConfig,
    app_name: &str,
) -> Result<()> {
    let response = client
        .get(format!("{}/apps/{}", FLY_API_URL, app_name))
        .header("Authorization", format!("Bearer {}", config.flyio.api_token))
        .send()
        .await?;

    if response.status().is_success() {
        return Ok(());
    }

    // Create new app
    let create_response = client
        .post(format!("{}/apps", FLY_API_URL))
        .header("Authorization", format!("Bearer {}", config.flyio.api_token))
        .json(&serde_json::json!({
            "app_name": app_name,
            "org_slug": config.flyio.org_slug
        }))
        .send()
        .await
        .context("Failed to create app")?;

    if !create_response.status().is_success() {
        let error_text = create_response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Failed to create app: {}", error_text));
    }

    Ok(())
}

async fn wait_for_machine(
    client: &reqwest::Client,
    config: &AppConfig,
    app_name: &str,
    machine_id: &str,
) -> Result<()> {
    for _ in 0..30 {
        let response = client
            .get(format!(
                "{}/apps/{}/machines/{}",
                FLY_API_URL, app_name, machine_id
            ))
            .header("Authorization", format!("Bearer {}", config.flyio.api_token))
            .send()
            .await?;

        if response.status().is_success() {
            let machine: MachineResponse = response.json().await?;
            if machine.state == "started" {
                return Ok(());
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    Err(anyhow::anyhow!("Machine failed to start in time"))
}
