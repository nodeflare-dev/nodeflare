use crate::models::{Announcement, CreateAnnouncement};
use chrono::{DateTime, Utc};
use mcp_common::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct AnnouncementRepository;

impl AnnouncementRepository {
    /// List active announcements that haven't expired
    pub async fn list_active(pool: &PgPool, limit: i64) -> Result<Vec<Announcement>> {
        let announcements = sqlx::query_as::<_, Announcement>(
            r#"
            SELECT id, title, content, type, is_active, published_at, expires_at, created_at, updated_at
            FROM announcements
            WHERE is_active = true
              AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY published_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(announcements)
    }

    /// List all announcements (for admin)
    pub async fn list_all(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<Announcement>> {
        let announcements = sqlx::query_as::<_, Announcement>(
            r#"
            SELECT id, title, content, type, is_active, published_at, expires_at, created_at, updated_at
            FROM announcements
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok(announcements)
    }

    /// Create a new announcement
    pub async fn create(pool: &PgPool, data: CreateAnnouncement) -> Result<Announcement> {
        let announcement = sqlx::query_as::<_, Announcement>(
            r#"
            INSERT INTO announcements (title, content, type, expires_at)
            VALUES ($1, $2, $3, $4)
            RETURNING id, title, content, type, is_active, published_at, expires_at, created_at, updated_at
            "#,
        )
        .bind(&data.title)
        .bind(&data.content)
        .bind(&data.announcement_type)
        .bind(data.expires_at)
        .fetch_one(pool)
        .await?;

        Ok(announcement)
    }

    /// Update an announcement
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        title: Option<String>,
        content: Option<String>,
        announcement_type: Option<String>,
        is_active: Option<bool>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Option<Announcement>> {
        let announcement = sqlx::query_as::<_, Announcement>(
            r#"
            UPDATE announcements
            SET
                title = COALESCE($2, title),
                content = COALESCE($3, content),
                type = COALESCE($4, type),
                is_active = COALESCE($5, is_active),
                expires_at = COALESCE($6, expires_at),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, title, content, type, is_active, published_at, expires_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(title)
        .bind(content)
        .bind(announcement_type)
        .bind(is_active)
        .bind(expires_at)
        .fetch_optional(pool)
        .await?;

        Ok(announcement)
    }

    /// Delete an announcement
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM announcements WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
