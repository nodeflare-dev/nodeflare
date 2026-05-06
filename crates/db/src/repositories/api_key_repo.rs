use crate::models::{ApiKey, CreateApiKey};
use mcp_common::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct ApiKeyRepository;

impl ApiKeyRepository {
    /// Maximum API keys per workspace to prevent resource exhaustion
    const MAX_KEYS_PER_WORKSPACE: i64 = 100;
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<ApiKey>> {
        let key = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT id, workspace_id, server_id, name, key_prefix, key_hash,
                   scopes, last_used_at, expires_at, created_at
            FROM api_keys
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(key)
    }

    pub async fn find_by_hash(pool: &PgPool, key_hash: &str) -> Result<Option<ApiKey>> {
        let key = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT id, workspace_id, server_id, name, key_prefix, key_hash,
                   scopes, last_used_at, expires_at, created_at
            FROM api_keys
            WHERE key_hash = $1
            "#,
        )
        .bind(key_hash)
        .fetch_optional(pool)
        .await?;

        Ok(key)
    }

    pub async fn list_by_workspace(pool: &PgPool, workspace_id: Uuid) -> Result<Vec<ApiKey>> {
        let keys = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT id, workspace_id, server_id, name, key_prefix, key_hash,
                   scopes, last_used_at, expires_at, created_at
            FROM api_keys
            WHERE workspace_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(workspace_id)
        .bind(Self::MAX_KEYS_PER_WORKSPACE)
        .fetch_all(pool)
        .await?;

        Ok(keys)
    }

    pub async fn list_by_server(pool: &PgPool, server_id: Uuid) -> Result<Vec<ApiKey>> {
        let keys = sqlx::query_as::<_, ApiKey>(
            r#"
            SELECT id, workspace_id, server_id, name, key_prefix, key_hash,
                   scopes, last_used_at, expires_at, created_at
            FROM api_keys
            WHERE server_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(server_id)
        .bind(Self::MAX_KEYS_PER_WORKSPACE)
        .fetch_all(pool)
        .await?;

        Ok(keys)
    }

    pub async fn create(pool: &PgPool, data: CreateApiKey) -> Result<ApiKey> {
        let scopes_json = serde_json::to_value(&data.scopes)?;

        let key = sqlx::query_as::<_, ApiKey>(
            r#"
            INSERT INTO api_keys (workspace_id, server_id, name, key_prefix, key_hash, scopes, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, workspace_id, server_id, name, key_prefix, key_hash,
                      scopes, last_used_at, expires_at, created_at
            "#,
        )
        .bind(data.workspace_id)
        .bind(data.server_id)
        .bind(&data.name)
        .bind(&data.key_prefix)
        .bind(&data.key_hash)
        .bind(scopes_json)
        .bind(data.expires_at)
        .fetch_one(pool)
        .await?;

        Ok(key)
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM api_keys WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn update_last_used(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn delete_expired(pool: &PgPool) -> Result<u64> {
        let result = sqlx::query("DELETE FROM api_keys WHERE expires_at < NOW()")
            .execute(pool)
            .await?;

        Ok(result.rows_affected())
    }

    pub async fn count_by_workspace(pool: &PgPool, workspace_id: Uuid) -> Result<i64> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM api_keys WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .fetch_one(pool)
        .await?;

        Ok(count.0)
    }
}
