//! GitHub App integration for MCP Cloud
//!
//! Handles:
//! - GitHub App authentication (JWT)
//! - Installation access tokens
//! - Repository operations (clone, get commit info)
//! - Webhook verification

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use mcp_common::AppConfig;
use serde::{Deserialize, Serialize};

const GITHUB_API_URL: &str = "https://api.github.com";

#[derive(Clone)]
pub struct GitHubApp {
    app_id: String,
    private_key: EncodingKey,
    http_client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct AppJwtClaims {
    iat: i64,
    exp: i64,
    iss: String,
}

#[derive(Debug, Deserialize)]
pub struct InstallationToken {
    pub token: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct Repository {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub default_branch: String,
    pub clone_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub message: Option<String>,
}

impl GitHubApp {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let private_key = EncodingKey::from_rsa_pem(config.github.app_private_key.as_bytes())
            .context("Invalid GitHub App private key")?;

        // SECURITY: Configure HTTP client with timeout and redirect policy
        let http_client = reqwest::Client::builder()
            .user_agent("MCP-Cloud/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;

        Ok(Self {
            app_id: config.github.app_id.clone(),
            private_key,
            http_client,
        })
    }

    fn generate_jwt(&self) -> Result<String> {
        let now = Utc::now();
        let claims = AppJwtClaims {
            iat: (now - Duration::seconds(60)).timestamp(),
            exp: (now + Duration::minutes(10)).timestamp(),
            iss: self.app_id.clone(),
        };

        let header = Header::new(Algorithm::RS256);
        encode(&header, &claims, &self.private_key).context("Failed to generate JWT")
    }

    pub async fn get_installation_token(&self, installation_id: i64) -> Result<InstallationToken> {
        let jwt = self.generate_jwt()?;

        let response = self
            .http_client
            .post(format!(
                "{}/app/installations/{}/access_tokens",
                GITHUB_API_URL, installation_id
            ))
            .header("Authorization", format!("Bearer {}", jwt))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .context("Failed to get installation token")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            // SECURITY: Log full error server-side, but don't expose sensitive details to caller
            tracing::error!("GitHub API error: {} - {}", status, body);
            anyhow::bail!("GitHub API error: {} - Failed to get installation token", status);
        }

        response.json().await.context("Failed to parse response")
    }

    pub async fn get_repository(
        &self,
        installation_id: i64,
        owner: &str,
        repo: &str,
    ) -> Result<Repository> {
        let token = self.get_installation_token(installation_id).await?;

        let response = self
            .http_client
            .get(format!("{}/repos/{}/{}", GITHUB_API_URL, owner, repo))
            .header("Authorization", format!("Bearer {}", token.token))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get repository");
        }

        response.json().await.context("Failed to parse repository")
    }

    pub async fn get_latest_commit(
        &self,
        installation_id: i64,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<Commit> {
        let token = self.get_installation_token(installation_id).await?;

        let response = self
            .http_client
            .get(format!(
                "{}/repos/{}/{}/commits/{}",
                GITHUB_API_URL, owner, repo, branch
            ))
            .header("Authorization", format!("Bearer {}", token.token))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get commit");
        }

        response.json().await.context("Failed to parse commit")
    }

    pub async fn download_tarball(
        &self,
        installation_id: i64,
        owner: &str,
        repo: &str,
        ref_name: &str,
    ) -> Result<Vec<u8>> {
        let token = self.get_installation_token(installation_id).await?;

        let response = self
            .http_client
            .get(format!(
                "{}/repos/{}/{}/tarball/{}",
                GITHUB_API_URL, owner, repo, ref_name
            ))
            .header("Authorization", format!("Bearer {}", token.token))
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download tarball");
        }

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .context("Failed to read tarball")
    }

    pub fn verify_webhook_signature(secret: &str, signature: &str, payload: &[u8]) -> bool {
        use ring::{hmac, constant_time};

        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let expected = hmac::sign(&key, payload);

        // signature format: sha256=<hex>
        if let Some(hex_sig) = signature.strip_prefix("sha256=") {
            if let Ok(sig_bytes) = hex::decode(hex_sig) {
                // Use constant-time comparison to prevent timing attacks
                return constant_time::verify_slices_are_equal(&sig_bytes, expected.as_ref()).is_ok();
            }
        }

        false
    }
}

#[derive(Debug, Deserialize)]
pub struct WebhookPayload {
    pub action: Option<String>,
    pub repository: Option<WebhookRepository>,
    pub installation: Option<WebhookInstallation>,
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookRepository {
    pub id: i64,
    pub full_name: String,
}

#[derive(Debug, Deserialize)]
pub struct WebhookInstallation {
    pub id: i64,
}

// MCP Validation types
#[derive(Debug, Clone)]
pub struct McpValidationResult {
    pub is_valid: bool,
    pub detected_runtime: Option<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubContent {
    content: Option<String>,
    encoding: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PackageJson {
    dependencies: Option<std::collections::HashMap<String, serde_json::Value>>,
    #[serde(rename = "devDependencies")]
    dev_dependencies: Option<std::collections::HashMap<String, serde_json::Value>>,
}

impl GitHubApp {
    /// Get file content from a repository
    pub async fn get_file_content(
        &self,
        installation_id: i64,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> Result<Option<String>> {
        let token = self.get_installation_token(installation_id).await?;

        let response = self
            .http_client
            .get(format!(
                "{}/repos/{}/{}/contents/{}?ref={}",
                GITHUB_API_URL, owner, repo, path, branch
            ))
            .header("Authorization", format!("Bearer {}", token.token))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            anyhow::bail!("Failed to get file content: {}", response.status());
        }

        let content: GitHubContent = response.json().await?;

        if let (Some(encoded), Some(encoding)) = (content.content, content.encoding) {
            if encoding == "base64" {
                let decoded = base64_decode(&encoded)?;
                return Ok(Some(decoded));
            }
        }

        Ok(None)
    }

    /// Check if a file exists in the repository
    pub async fn file_exists(
        &self,
        installation_id: i64,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> Result<bool> {
        let token = self.get_installation_token(installation_id).await?;

        let response = self
            .http_client
            .head(format!(
                "{}/repos/{}/{}/contents/{}?ref={}",
                GITHUB_API_URL, owner, repo, path, branch
            ))
            .header("Authorization", format!("Bearer {}", token.token))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    /// Validate if a repository is a valid MCP server
    pub async fn validate_mcp_repository(
        &self,
        installation_id: i64,
        owner: &str,
        repo: &str,
        branch: &str,
        expected_runtime: Option<&str>,
    ) -> Result<McpValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut detected_runtime: Option<String> = None;

        // Check for Node.js MCP server
        let has_package_json = self.file_exists(installation_id, owner, repo, "package.json", branch).await.unwrap_or(false);

        // Check for Python MCP server
        let has_requirements = self.file_exists(installation_id, owner, repo, "requirements.txt", branch).await.unwrap_or(false);
        let has_pyproject = self.file_exists(installation_id, owner, repo, "pyproject.toml", branch).await.unwrap_or(false);

        // Check for Go MCP server
        let has_go_mod = self.file_exists(installation_id, owner, repo, "go.mod", branch).await.unwrap_or(false);

        // Check for Rust MCP server
        let has_cargo_toml = self.file_exists(installation_id, owner, repo, "Cargo.toml", branch).await.unwrap_or(false);

        // Detect runtime (priority: explicit expected_runtime > detected from files)
        if has_package_json && (expected_runtime.is_none() || expected_runtime == Some("node")) {
            detected_runtime = Some("node".to_string());

            // Validate Node.js MCP dependencies
            if let Ok(Some(content)) = self.get_file_content(installation_id, owner, repo, "package.json", branch).await {
                match serde_json::from_str::<PackageJson>(&content) {
                    Ok(pkg) => {
                        let has_mcp_sdk = pkg.dependencies
                            .as_ref()
                            .map(|d| d.contains_key("@modelcontextprotocol/sdk"))
                            .unwrap_or(false)
                            || pkg.dev_dependencies
                                .as_ref()
                                .map(|d| d.contains_key("@modelcontextprotocol/sdk"))
                                .unwrap_or(false);

                        if !has_mcp_sdk {
                            warnings.push("package.json does not contain @modelcontextprotocol/sdk dependency. Make sure this is a valid MCP server.".to_string());
                        }
                    }
                    Err(_) => {
                        errors.push("Invalid package.json format".to_string());
                    }
                }
            }
        } else if (has_requirements || has_pyproject) && (expected_runtime.is_none() || expected_runtime == Some("python")) {
            detected_runtime = Some("python".to_string());

            // Validate Python MCP dependencies
            if has_requirements {
                if let Ok(Some(content)) = self.get_file_content(installation_id, owner, repo, "requirements.txt", branch).await {
                    let has_mcp = content.lines().any(|line| {
                        let line = line.trim().to_lowercase();
                        line.starts_with("mcp") || line.contains("mcp>=") || line.contains("mcp==")
                    });
                    if !has_mcp {
                        warnings.push("requirements.txt does not contain 'mcp' package. Make sure this is a valid MCP server.".to_string());
                    }
                }
            }

            if has_pyproject {
                if let Ok(Some(content)) = self.get_file_content(installation_id, owner, repo, "pyproject.toml", branch).await {
                    let has_mcp = content.contains("mcp");
                    if !has_mcp {
                        warnings.push("pyproject.toml does not reference 'mcp' package. Make sure this is a valid MCP server.".to_string());
                    }
                }
            }
        } else if has_go_mod && (expected_runtime.is_none() || expected_runtime == Some("go")) {
            detected_runtime = Some("go".to_string());

            // Validate Go MCP dependencies
            if let Ok(Some(content)) = self.get_file_content(installation_id, owner, repo, "go.mod", branch).await {
                // Check for MCP-related Go packages
                let has_mcp = content.contains("mcp") || content.contains("model-context-protocol");
                if !has_mcp {
                    warnings.push("go.mod does not contain MCP-related dependencies. Make sure this is a valid MCP server.".to_string());
                }
            }
        } else if has_cargo_toml && (expected_runtime.is_none() || expected_runtime == Some("rust")) {
            detected_runtime = Some("rust".to_string());

            // Validate Rust MCP dependencies
            if let Ok(Some(content)) = self.get_file_content(installation_id, owner, repo, "Cargo.toml", branch).await {
                // Check for MCP-related Rust crates
                let has_mcp = content.contains("mcp") || content.contains("model-context-protocol");
                if !has_mcp {
                    warnings.push("Cargo.toml does not contain MCP-related dependencies. Make sure this is a valid MCP server.".to_string());
                }
            }
        } else if expected_runtime == Some("docker") {
            // Docker runtime - check for Dockerfile
            let has_dockerfile = self.file_exists(installation_id, owner, repo, "Dockerfile", branch).await.unwrap_or(false);
            if has_dockerfile {
                detected_runtime = Some("docker".to_string());
            } else {
                errors.push("No Dockerfile found for docker runtime.".to_string());
            }
        } else {
            errors.push("No package.json, requirements.txt, pyproject.toml, go.mod, or Cargo.toml found. Cannot determine project type.".to_string());
        }

        // Validate expected runtime matches detected
        if let (Some(expected), Some(ref detected)) = (expected_runtime, &detected_runtime) {
            if expected != detected && expected != "docker" {
                errors.push(format!(
                    "Runtime mismatch: expected '{}' but detected '{}'",
                    expected, detected
                ));
            }
        }

        let is_valid = errors.is_empty();

        Ok(McpValidationResult {
            is_valid,
            detected_runtime,
            errors,
            warnings,
        })
    }
}

/// Helper function to decode base64 content from GitHub API
fn base64_decode(encoded: &str) -> Result<String> {
    use base64::{Engine as _, engine::general_purpose};

    // GitHub API returns base64 with newlines, remove them
    let cleaned = encoded.replace('\n', "").replace('\r', "");
    let bytes = general_purpose::STANDARD.decode(&cleaned)
        .context("Failed to decode base64 content")?;
    String::from_utf8(bytes).context("Invalid UTF-8 in file content")
}
