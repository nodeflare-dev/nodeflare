use crate::models::{CreateDeployWebhook, DeployWebhook, UpdateDeployWebhook};
use mcp_common::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct DeployWebhookRepository;

impl DeployWebhookRepository {
    /// List all webhooks for a server
    pub async fn list_by_server(pool: &PgPool, server_id: Uuid) -> Result<Vec<DeployWebhook>> {
        let webhooks = sqlx::query_as::<_, DeployWebhook>(
            r#"
            SELECT id, server_id, name, webhook_url, webhook_type, events, secret,
                   encrypted_secret, secret_nonce,
                   is_active, last_triggered_at, last_status, created_at, updated_at
            FROM deploy_webhooks
            WHERE server_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(server_id)
        .fetch_all(pool)
        .await?;

        Ok(webhooks)
    }

    /// List active webhooks for a server matching an event
    pub async fn list_active_for_event(
        pool: &PgPool,
        server_id: Uuid,
        event: &str,
    ) -> Result<Vec<DeployWebhook>> {
        let webhooks = sqlx::query_as::<_, DeployWebhook>(
            r#"
            SELECT id, server_id, name, webhook_url, webhook_type, events, secret,
                   encrypted_secret, secret_nonce,
                   is_active, last_triggered_at, last_status, created_at, updated_at
            FROM deploy_webhooks
            WHERE server_id = $1 AND is_active = true AND $2 = ANY(events)
            "#,
        )
        .bind(server_id)
        .bind(event)
        .fetch_all(pool)
        .await?;

        Ok(webhooks)
    }

    /// Get webhook by ID
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<DeployWebhook>> {
        let webhook = sqlx::query_as::<_, DeployWebhook>(
            r#"
            SELECT id, server_id, name, webhook_url, webhook_type, events, secret,
                   encrypted_secret, secret_nonce,
                   is_active, last_triggered_at, last_status, created_at, updated_at
            FROM deploy_webhooks
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(webhook)
    }

    /// Create a new webhook with encrypted secret
    pub async fn create(pool: &PgPool, data: CreateDeployWebhook) -> Result<DeployWebhook> {
        let webhook = sqlx::query_as::<_, DeployWebhook>(
            r#"
            INSERT INTO deploy_webhooks (server_id, name, webhook_url, webhook_type, events, encrypted_secret, secret_nonce)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, server_id, name, webhook_url, webhook_type, events, secret,
                      encrypted_secret, secret_nonce,
                      is_active, last_triggered_at, last_status, created_at, updated_at
            "#,
        )
        .bind(data.server_id)
        .bind(&data.name)
        .bind(&data.webhook_url)
        .bind(&data.webhook_type)
        .bind(&data.events)
        .bind(&data.encrypted_secret)
        .bind(&data.secret_nonce)
        .fetch_one(pool)
        .await?;

        Ok(webhook)
    }

    /// Update a webhook with encrypted secret
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        data: UpdateDeployWebhook,
    ) -> Result<Option<DeployWebhook>> {
        let webhook = sqlx::query_as::<_, DeployWebhook>(
            r#"
            UPDATE deploy_webhooks
            SET
                name = COALESCE($2, name),
                webhook_url = COALESCE($3, webhook_url),
                events = COALESCE($4, events),
                encrypted_secret = COALESCE($5, encrypted_secret),
                secret_nonce = COALESCE($6, secret_nonce),
                is_active = COALESCE($7, is_active),
                secret = NULL,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, server_id, name, webhook_url, webhook_type, events, secret,
                      encrypted_secret, secret_nonce,
                      is_active, last_triggered_at, last_status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(data.name)
        .bind(data.webhook_url)
        .bind(data.events)
        .bind(data.encrypted_secret)
        .bind(data.secret_nonce)
        .bind(data.is_active)
        .fetch_optional(pool)
        .await?;

        Ok(webhook)
    }

    /// Update webhook trigger status
    pub async fn update_trigger_status(
        pool: &PgPool,
        id: Uuid,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deploy_webhooks
            SET last_triggered_at = NOW(), last_status = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Delete a webhook
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM deploy_webhooks WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
