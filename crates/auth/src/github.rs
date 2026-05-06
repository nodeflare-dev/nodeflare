use mcp_common::{AppConfig, Error, Result};
use serde::{Deserialize, Serialize};

const GITHUB_AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_API_URL: &str = "https://api.github.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Clone)]
pub struct GitHubOAuth {
    client_id: String,
    client_secret: String,
    redirect_url: String,
    http_client: reqwest::Client,
}

impl GitHubOAuth {
    pub fn new(config: &AppConfig, redirect_url: &str) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .user_agent("MCP-Cloud/1.0")
            .build()
            .map_err(|e| Error::Internal(e.to_string()))?;

        Ok(Self {
            client_id: config.github.client_id.clone(),
            client_secret: config.github.client_secret.clone(),
            redirect_url: redirect_url.to_string(),
            http_client,
        })
    }

    pub fn get_authorization_url(&self) -> (String, String) {
        // SECURITY: Use cryptographically secure random token instead of UUID
        // 32 bytes = 256 bits of entropy, hex-encoded to 64 characters
        let state = crate::jwt::generate_random_token(32)
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string()); // Fallback to UUID if RNG fails
        let redirect_encoded = url::form_urlencoded::byte_serialize(self.redirect_url.as_bytes())
            .collect::<String>();
        let url = format!(
            "{}?client_id={}&redirect_uri={}&scope=user:email%20read:user&state={}",
            GITHUB_AUTH_URL,
            &self.client_id,
            redirect_encoded,
            &state
        );
        (url, state)
    }

    pub async fn exchange_code(&self, code: &str) -> Result<String> {
        let response = self
            .http_client
            .post(GITHUB_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
                ("code", &code.to_string()),
                ("redirect_uri", &self.redirect_url),
            ])
            .send()
            .await
            .map_err(|e| Error::ExternalService(format!("GitHub OAuth error: {}", e)))?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::ExternalService(format!(
                "GitHub OAuth error: {}",
                body
            )));
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| Error::ExternalService(format!("Failed to parse token: {}", e)))?;

        Ok(token_response.access_token)
    }

    pub async fn get_user(&self, access_token: &str) -> Result<GitHubUser> {
        let response = self
            .http_client
            .get(format!("{}/user", GITHUB_API_URL))
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| Error::ExternalService(format!("GitHub API error: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::ExternalService(format!(
                "GitHub API error: {} - {}",
                status, body
            )));
        }

        response
            .json::<GitHubUser>()
            .await
            .map_err(|e| Error::ExternalService(format!("Failed to parse user: {}", e)))
    }

    pub async fn get_primary_email(&self, access_token: &str) -> Result<Option<String>> {
        let response = self
            .http_client
            .get(format!("{}/user/emails", GITHUB_API_URL))
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| Error::ExternalService(format!("GitHub API error: {}", e)))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let emails: Vec<GitHubEmail> = response
            .json()
            .await
            .map_err(|e| Error::ExternalService(format!("Failed to parse emails: {}", e)))?;

        Ok(emails
            .into_iter()
            .find(|e| e.primary && e.verified)
            .map(|e| e.email))
    }
}
