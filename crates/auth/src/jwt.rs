use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use mcp_common::{AppConfig, Error, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,       // User ID
    pub exp: i64,          // Expiration time
    pub iat: i64,          // Issued at
    pub jti: String,       // JWT ID for token revocation
    pub workspace_id: Option<String>, // Current workspace
}

impl Claims {
    pub fn user_id(&self) -> Result<Uuid> {
        Uuid::parse_str(&self.sub).map_err(|_| Error::Unauthorized)
    }

    pub fn workspace_id(&self) -> Option<Uuid> {
        self.workspace_id
            .as_ref()
            .and_then(|id| Uuid::parse_str(id).ok())
    }
}

#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    expiration_hours: i64,
}

impl JwtService {
    pub fn new(config: &AppConfig) -> Self {
        let secret = config.auth.jwt_secret.as_bytes();
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            expiration_hours: config.auth.jwt_expiration_hours,
        }
    }

    pub fn generate_token(&self, user_id: Uuid, workspace_id: Option<Uuid>) -> Result<String> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.expiration_hours);
        // Generate unique JWT ID for token revocation support
        let jti = Uuid::new_v4().to_string();

        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            jti,
            workspace_id: workspace_id.map(|id| id.to_string()),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)?;
        Ok(token)
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let token_data: TokenData<Claims> =
            decode(token, &self.decoding_key, &Validation::default())?;

        Ok(token_data.claims)
    }

    pub fn refresh_token(&self, token: &str) -> Result<String> {
        let claims = self.verify_token(token)?;
        let user_id = claims.user_id()?;
        self.generate_token(user_id, claims.workspace_id())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    pub token: String,
    pub user_id: Uuid,
    pub expires_at: chrono::DateTime<Utc>,
}

impl RefreshToken {
    pub fn generate(user_id: Uuid, expiration_days: i64) -> Result<Self> {
        let token = generate_random_token(32)?;
        let expires_at = Utc::now() + Duration::days(expiration_days);

        Ok(Self {
            token,
            user_id,
            expires_at,
        })
    }

    pub fn hash(&self) -> String {
        use ring::digest::{digest, SHA256};
        let hash = digest(&SHA256, self.token.as_bytes());
        hex::encode(hash.as_ref())
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at < Utc::now()
    }
}

pub fn generate_random_token(length: usize) -> Result<String> {
    use ring::rand::{SecureRandom, SystemRandom};
    let rng = SystemRandom::new();
    let mut bytes = vec![0u8; length];
    rng.fill(&mut bytes)
        .map_err(|_| Error::Internal("Failed to generate random bytes".into()))?;
    Ok(hex::encode(bytes))
}

pub fn hash_token(token: &str) -> String {
    use ring::digest::{digest, SHA256};
    let hash = digest(&SHA256, token.as_bytes());
    hex::encode(hash.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AppConfig {
        AppConfig {
            auth: mcp_common::config::AuthConfig {
                jwt_secret: "test-secret-key-for-testing-only".to_string(),
                jwt_expiration_hours: 24,
                refresh_token_expiration_days: 30,
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_generate_and_verify_token() {
        let service = JwtService::new(&test_config());
        let user_id = Uuid::new_v4();

        let token = service.generate_token(user_id, None).unwrap();
        let claims = service.verify_token(&token).unwrap();

        assert_eq!(claims.user_id().unwrap(), user_id);
    }
}
