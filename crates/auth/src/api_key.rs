use chrono::{Duration, Utc};
use mcp_common::{Error, Result};
use mcp_db::models::CreateApiKey;
use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};
use subtle::ConstantTimeEq;
use uuid::Uuid;

const API_KEY_PREFIX: &str = "mcp_";
const API_KEY_LENGTH: usize = 32;

#[derive(Debug, Clone)]
pub struct GeneratedApiKey {
    pub full_key: String,
    pub prefix: String,
    pub hash: String,
}

pub struct ApiKeyService;

impl ApiKeyService {
    pub fn generate() -> Result<GeneratedApiKey> {
        let rng = SystemRandom::new();

        // Generate random bytes using cryptographically secure RNG
        let mut random_bytes = vec![0u8; API_KEY_LENGTH];
        rng.fill(&mut random_bytes)
            .map_err(|_| Error::Internal("Failed to generate random bytes for API key".into()))?;
        let random_part = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            &random_bytes,
        );

        // Create full key with prefix
        let full_key = format!("{}{}", API_KEY_PREFIX, random_part);

        // Extract prefix (first 8 chars after mcp_)
        let prefix = full_key.chars().take(12).collect::<String>();

        // Create hash
        let hash = Self::hash_key(&full_key);

        Ok(GeneratedApiKey {
            full_key,
            prefix,
            hash,
        })
    }

    pub fn hash_key(key: &str) -> String {
        let hash = digest(&SHA256, key.as_bytes());
        hex::encode(hash.as_ref())
    }

    /// Verify an API key against its hash using constant-time comparison.
    /// This prevents timing attacks that could be used to guess valid API keys.
    pub fn verify(key: &str, hash: &str) -> bool {
        let computed_hash = Self::hash_key(key);
        // Use constant-time comparison to prevent timing attacks
        computed_hash.as_bytes().ct_eq(hash.as_bytes()).into()
    }

    pub fn create_api_key_data(
        workspace_id: Uuid,
        server_id: Option<Uuid>,
        name: String,
        scopes: Vec<String>,
        expires_in_days: Option<i64>,
    ) -> Result<(CreateApiKey, String)> {
        let generated = Self::generate()?;

        let expires_at = expires_in_days.map(|days| Utc::now() + Duration::days(days));

        let data = CreateApiKey {
            workspace_id,
            server_id,
            name,
            key_prefix: generated.prefix.clone(),
            key_hash: generated.hash,
            scopes,
            expires_at,
        };

        Ok((data, generated.full_key))
    }

    pub fn is_valid_format(key: &str) -> bool {
        key.starts_with(API_KEY_PREFIX) && key.len() > 12
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_api_key() {
        let key = ApiKeyService::generate().unwrap();

        assert!(key.full_key.starts_with("mcp_"));
        assert_eq!(key.prefix.len(), 12);
        assert_eq!(key.hash.len(), 64); // SHA256 hex = 64 chars
    }

    #[test]
    fn test_verify_api_key() {
        let key = ApiKeyService::generate().unwrap();

        assert!(ApiKeyService::verify(&key.full_key, &key.hash));
        assert!(!ApiKeyService::verify("wrong_key", &key.hash));
    }

    #[test]
    fn test_is_valid_format() {
        assert!(ApiKeyService::is_valid_format("mcp_abc123def456"));
        assert!(!ApiKeyService::is_valid_format("invalid_key"));
        assert!(!ApiKeyService::is_valid_format("mcp_"));
    }
}
