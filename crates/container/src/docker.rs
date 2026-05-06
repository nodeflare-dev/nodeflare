use crate::{Container, ContainerConfig, ContainerRuntime, ContainerStatus};
use anyhow::{Context, Result};
use bollard::container::{Config, CreateContainerOptions, LogsOptions, StartContainerOptions};
use bollard::Docker;
use futures::StreamExt;
use std::collections::HashMap;

pub struct DockerRuntime {
    client: Docker,
}

impl DockerRuntime {
    pub fn new() -> Result<Self> {
        let client = Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker")?;
        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl ContainerRuntime for DockerRuntime {
    async fn create(&self, name: &str, config: ContainerConfig) -> Result<Container> {
        let env: Vec<String> = config
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        let container_config = Config {
            image: Some(config.image.clone()),
            env: Some(env),
            exposed_ports: Some(HashMap::from([(
                format!("{}/tcp", config.port),
                HashMap::new(),
            )])),
            host_config: Some(bollard::service::HostConfig {
                memory: Some((config.memory_mb as i64) * 1024 * 1024),
                cpu_shares: Some(config.cpu_shares as i64),
                port_bindings: Some(HashMap::from([(
                    format!("{}/tcp", config.port),
                    Some(vec![bollard::service::PortBinding {
                        host_ip: Some("0.0.0.0".to_string()),
                        host_port: Some("0".to_string()), // Random port
                    }]),
                )])),
                ..Default::default()
            }),
            ..Default::default()
        };

        let response = self
            .client
            .create_container(
                Some(CreateContainerOptions { name, platform: None }),
                container_config,
            )
            .await
            .context("Failed to create container")?;

        Ok(Container {
            id: response.id,
            name: name.to_string(),
            status: ContainerStatus::Creating,
            endpoint_url: None,
        })
    }

    async fn start(&self, id: &str) -> Result<()> {
        self.client
            .start_container(id, None::<StartContainerOptions<String>>)
            .await
            .context("Failed to start container")?;
        Ok(())
    }

    async fn stop(&self, id: &str) -> Result<()> {
        self.client
            .stop_container(id, None)
            .await
            .context("Failed to stop container")?;
        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.client
            .remove_container(id, None)
            .await
            .context("Failed to remove container")?;
        Ok(())
    }

    async fn status(&self, id: &str) -> Result<ContainerStatus> {
        let inspect = self
            .client
            .inspect_container(id, None)
            .await
            .context("Failed to inspect container")?;

        let status = match inspect.state.and_then(|s| s.status) {
            Some(bollard::service::ContainerStateStatusEnum::RUNNING) => ContainerStatus::Running,
            Some(bollard::service::ContainerStateStatusEnum::EXITED) => ContainerStatus::Stopped,
            Some(bollard::service::ContainerStateStatusEnum::DEAD) => ContainerStatus::Failed,
            _ => ContainerStatus::Creating,
        };

        Ok(status)
    }

    async fn logs(&self, id: &str, tail: usize) -> Result<String> {
        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: tail.to_string(),
            ..Default::default()
        };

        let mut stream = self.client.logs(id, Some(options));
        let mut logs = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(output) => {
                    logs.push_str(&output.to_string());
                }
                Err(e) => {
                    tracing::warn!("Error reading logs: {}", e);
                    break;
                }
            }
        }

        Ok(logs)
    }
}
