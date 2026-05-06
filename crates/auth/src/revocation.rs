//! Token revocation list stored in Redis.
//!
//! This module provides functions to revoke JWT tokens and check if a token
//! has been revoked. Revoked tokens are stored in Redis with a TTL matching
//! the token's remaining lifetime.

use fred::interfaces::KeysInterface;
use fred::prelude::RedisClient;

/// Redis key prefix for revoked tokens
const REVOKED_TOKEN_PREFIX: &str = "revoked_token:";

/// Add a token's JTI to the revocation list.
///
/// The TTL should be set to the remaining lifetime of the token.
/// After the token expires naturally, there's no need to keep it in the revocation list.
pub async fn revoke_token(redis: &RedisClient, jti: &str, ttl_secs: i64) -> Result<(), fred::error::RedisError> {
    if ttl_secs <= 0 {
        // Token already expired, no need to revoke
        return Ok(());
    }

    let key = format!("{}{}", REVOKED_TOKEN_PREFIX, jti);
    redis
        .set::<(), _, _>(
            &key,
            "1",
            Some(fred::types::Expiration::EX(ttl_secs)),
            None,
            false,
        )
        .await
}

/// Check if a token has been revoked.
///
/// Returns true if the token is in the revocation list, false otherwise.
pub async fn is_token_revoked(redis: &RedisClient, jti: &str) -> bool {
    let key = format!("{}{}", REVOKED_TOKEN_PREFIX, jti);
    redis.exists::<i64, _>(&key).await.unwrap_or(0) > 0
}

/// Revoke all tokens for a user by storing their user ID with a flag.
/// This is a supplementary mechanism - individual tokens should still be revoked
/// when possible for better security.
pub async fn revoke_all_user_tokens(
    redis: &RedisClient,
    user_id: &str,
    ttl_secs: i64,
) -> Result<(), fred::error::RedisError> {
    let key = format!("{}user:{}", REVOKED_TOKEN_PREFIX, user_id);
    let timestamp = chrono::Utc::now().timestamp();
    redis
        .set::<(), _, _>(
            &key,
            timestamp.to_string(),
            Some(fred::types::Expiration::EX(ttl_secs)),
            None,
            false,
        )
        .await
}

/// Check if all tokens for a user have been revoked since a given timestamp.
/// Returns the revocation timestamp if found, None otherwise.
pub async fn get_user_revocation_timestamp(
    redis: &RedisClient,
    user_id: &str,
) -> Option<i64> {
    let key = format!("{}user:{}", REVOKED_TOKEN_PREFIX, user_id);
    redis
        .get::<Option<String>, _>(&key)
        .await
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok())
}
