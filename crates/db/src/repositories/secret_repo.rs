use crate::models::{CreateSecret, Secret, UpdateSecret};
use mcp_common::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct SecretRepository;

impl SecretRepository {
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Secret>> {
        let secret = sqlx::query_as::<_, Secret>(
            r#"
            SELECT id, server_id, key, encrypted_value, nonce, created_at, updated_at
            FROM secrets
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(secret)
    }

    pub async fn find_by_key(
        pool: &PgPool,
        server_id: Uuid,
        key: &str,
    ) -> Result<Option<Secret>> {
        let secret = sqlx::query_as::<_, Secret>(
            r#"
            SELECT id, server_id, key, encrypted_value, nonce, created_at, updated_at
            FROM secrets
            WHERE server_id = $1 AND key = $2
            "#,
        )
        .bind(server_id)
        .bind(key)
        .fetch_optional(pool)
        .await?;

        Ok(secret)
    }

    /// Maximum secrets per server to prevent resource exhaustion
    const MAX_SECRETS_PER_SERVER: i64 = 100;

    pub async fn list_by_server(pool: &PgPool, server_id: Uuid) -> Result<Vec<Secret>> {
        let secrets = sqlx::query_as::<_, Secret>(
            r#"
            SELECT id, server_id, key, encrypted_value, nonce, created_at, updated_at
            FROM secrets
            WHERE server_id = $1
            ORDER BY key
            LIMIT $2
            "#,
        )
        .bind(server_id)
        .bind(Self::MAX_SECRETS_PER_SERVER)
        .fetch_all(pool)
        .await?;

        Ok(secrets)
    }

    pub async fn list_keys_by_server(pool: &PgPool, server_id: Uuid) -> Result<Vec<String>> {
        let keys: Vec<String> = sqlx::query_scalar(
            "SELECT key FROM secrets WHERE server_id = $1 ORDER BY key LIMIT $2",
        )
        .bind(server_id)
        .bind(Self::MAX_SECRETS_PER_SERVER)
        .fetch_all(pool)
        .await?;

        Ok(keys)
    }

    pub async fn create(pool: &PgPool, data: CreateSecret) -> Result<Secret> {
        let secret = sqlx::query_as::<_, Secret>(
            r#"
            INSERT INTO secrets (server_id, key, encrypted_value, nonce)
            VALUES ($1, $2, $3, $4)
            RETURNING id, server_id, key, encrypted_value, nonce, created_at, updated_at
            "#,
        )
        .bind(data.server_id)
        .bind(&data.key)
        .bind(&data.encrypted_value)
        .bind(&data.nonce)
        .fetch_one(pool)
        .await?;

        Ok(secret)
    }

    pub async fn upsert(pool: &PgPool, data: CreateSecret) -> Result<Secret> {
        let secret = sqlx::query_as::<_, Secret>(
            r#"
            INSERT INTO secrets (server_id, key, encrypted_value, nonce)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (server_id, key) DO UPDATE SET
                encrypted_value = EXCLUDED.encrypted_value,
                nonce = EXCLUDED.nonce,
                updated_at = NOW()
            RETURNING id, server_id, key, encrypted_value, nonce, created_at, updated_at
            "#,
        )
        .bind(data.server_id)
        .bind(&data.key)
        .bind(&data.encrypted_value)
        .bind(&data.nonce)
        .fetch_one(pool)
        .await?;

        Ok(secret)
    }

    pub async fn update(pool: &PgPool, id: Uuid, data: UpdateSecret) -> Result<Secret> {
        let secret = sqlx::query_as::<_, Secret>(
            r#"
            UPDATE secrets
            SET
                encrypted_value = $2,
                nonce = $3,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, server_id, key, encrypted_value, nonce, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&data.encrypted_value)
        .bind(&data.nonce)
        .fetch_one(pool)
        .await?;

        Ok(secret)
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM secrets WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn delete_by_key(pool: &PgPool, server_id: Uuid, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM secrets WHERE server_id = $1 AND key = $2")
            .bind(server_id)
            .bind(key)
            .execute(pool)
            .await?;

        Ok(())
    }
}
