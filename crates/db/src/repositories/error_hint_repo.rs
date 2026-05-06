use crate::models::{CreateErrorHint, ErrorHint, UpdateErrorHint};
use mcp_common::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct ErrorHintRepository;

impl ErrorHintRepository {
    /// List all active error hints for a specific locale, ordered by priority (highest first)
    pub async fn list_active(pool: &PgPool, locale: &str) -> Result<Vec<ErrorHint>> {
        let hints = sqlx::query_as::<_, ErrorHint>(
            r#"
            SELECT id, keywords, hint_message, priority, category, locale, is_active, created_at, updated_at
            FROM error_hints
            WHERE is_active = true AND locale = $1
            ORDER BY priority DESC
            "#,
        )
        .bind(locale)
        .fetch_all(pool)
        .await?;

        Ok(hints)
    }

    /// List all error hints (for admin)
    pub async fn list_all(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<ErrorHint>> {
        let hints = sqlx::query_as::<_, ErrorHint>(
            r#"
            SELECT id, keywords, hint_message, priority, category, locale, is_active, created_at, updated_at
            FROM error_hints
            ORDER BY locale, priority DESC, created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok(hints)
    }

    /// Find matching hint for an error message with locale support
    /// Falls back to 'en' if no hint found for the specified locale
    /// Returns the highest priority hint where ALL keywords match (case-insensitive)
    pub async fn find_matching_hint(pool: &PgPool, error_msg: &str, locale: &str) -> Result<Option<ErrorHint>> {
        let error_lower = error_msg.to_lowercase();

        // First try to find a hint in the user's locale
        let hints = Self::list_active(pool, locale).await?;

        for hint in &hints {
            let all_match = hint.keywords.iter().all(|keyword| {
                error_lower.contains(&keyword.to_lowercase())
            });

            if all_match {
                return Ok(Some(hint.clone()));
            }
        }

        // If no match found and locale is not 'en', fall back to English
        if locale != "en" {
            let en_hints = Self::list_active(pool, "en").await?;

            for hint in en_hints {
                let all_match = hint.keywords.iter().all(|keyword| {
                    error_lower.contains(&keyword.to_lowercase())
                });

                if all_match {
                    return Ok(Some(hint));
                }
            }
        }

        Ok(None)
    }

    /// Create a new error hint
    pub async fn create(pool: &PgPool, data: CreateErrorHint) -> Result<ErrorHint> {
        let hint = sqlx::query_as::<_, ErrorHint>(
            r#"
            INSERT INTO error_hints (keywords, hint_message, priority, category, locale)
            VALUES ($1, $2, COALESCE($3, 0), COALESCE($4, 'general'), COALESCE($5, 'en'))
            RETURNING id, keywords, hint_message, priority, category, locale, is_active, created_at, updated_at
            "#,
        )
        .bind(&data.keywords)
        .bind(&data.hint_message)
        .bind(data.priority)
        .bind(&data.category)
        .bind(&data.locale)
        .fetch_one(pool)
        .await?;

        Ok(hint)
    }

    /// Update an error hint
    pub async fn update(pool: &PgPool, id: Uuid, data: UpdateErrorHint) -> Result<Option<ErrorHint>> {
        let hint = sqlx::query_as::<_, ErrorHint>(
            r#"
            UPDATE error_hints
            SET
                keywords = COALESCE($2, keywords),
                hint_message = COALESCE($3, hint_message),
                priority = COALESCE($4, priority),
                category = COALESCE($5, category),
                locale = COALESCE($6, locale),
                is_active = COALESCE($7, is_active),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, keywords, hint_message, priority, category, locale, is_active, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&data.keywords)
        .bind(&data.hint_message)
        .bind(data.priority)
        .bind(&data.category)
        .bind(&data.locale)
        .bind(data.is_active)
        .fetch_optional(pool)
        .await?;

        Ok(hint)
    }

    /// Delete an error hint
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM error_hints WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get an error hint by ID
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<ErrorHint>> {
        let hint = sqlx::query_as::<_, ErrorHint>(
            r#"
            SELECT id, keywords, hint_message, priority, category, locale, is_active, created_at, updated_at
            FROM error_hints
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(hint)
    }

    /// Count total hints
    pub async fn count(pool: &PgPool) -> Result<i64> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM error_hints")
            .fetch_one(pool)
            .await?;

        Ok(count.0)
    }
}
