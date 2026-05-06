use crate::models::{ContactMessage, CreateContactMessage};
use mcp_common::Result;
use sqlx::PgPool;

pub struct ContactMessageRepository;

impl ContactMessageRepository {
    pub async fn create(pool: &PgPool, data: CreateContactMessage) -> Result<ContactMessage> {
        let message = sqlx::query_as::<_, ContactMessage>(
            r#"
            INSERT INTO contact_messages (name, email, message)
            VALUES ($1, $2, $3)
            RETURNING id, name, email, message, created_at
            "#,
        )
        .bind(&data.name)
        .bind(&data.email)
        .bind(&data.message)
        .fetch_one(pool)
        .await?;

        Ok(message)
    }

    pub async fn list(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<ContactMessage>> {
        let messages = sqlx::query_as::<_, ContactMessage>(
            r#"
            SELECT id, name, email, message, created_at
            FROM contact_messages
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok(messages)
    }
}
