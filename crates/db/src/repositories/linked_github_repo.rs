use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{CreateLinkedGitHubAccount, LinkedGitHubAccount, LinkedGitHubAccountWithToken};
use mcp_common::Result;

pub struct LinkedGitHubAccountRepository;

impl LinkedGitHubAccountRepository {
    /// List all linked GitHub accounts for a user (without tokens)
    pub async fn list_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<LinkedGitHubAccount>> {
        let accounts = sqlx::query_as::<_, LinkedGitHubAccount>(
            r#"
            SELECT id, user_id, github_id, github_username, github_avatar_url,
                   scopes, is_primary, created_at, updated_at
            FROM linked_github_accounts
            WHERE user_id = $1
            ORDER BY is_primary DESC, created_at ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(accounts)
    }

    /// Get a specific linked account with token by ID (for API access)
    pub async fn get_with_token(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<LinkedGitHubAccountWithToken>> {
        let account = sqlx::query_as::<_, LinkedGitHubAccountWithToken>(
            r#"
            SELECT id, user_id, github_id, github_username, github_avatar_url,
                   access_token_encrypted, access_token_nonce, scopes, is_primary,
                   created_at, updated_at
            FROM linked_github_accounts
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(account)
    }

    /// Get the primary linked account with token for a user
    pub async fn get_primary_with_token(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Option<LinkedGitHubAccountWithToken>> {
        let account = sqlx::query_as::<_, LinkedGitHubAccountWithToken>(
            r#"
            SELECT id, user_id, github_id, github_username, github_avatar_url,
                   access_token_encrypted, access_token_nonce, scopes, is_primary,
                   created_at, updated_at
            FROM linked_github_accounts
            WHERE user_id = $1 AND is_primary = true
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(account)
    }

    /// Get any linked account with token for a user (fallback if no primary)
    pub async fn get_any_with_token(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Option<LinkedGitHubAccountWithToken>> {
        // First try to get primary
        if let Some(account) = Self::get_primary_with_token(pool, user_id).await? {
            return Ok(Some(account));
        }

        // Fall back to any account
        let account = sqlx::query_as::<_, LinkedGitHubAccountWithToken>(
            r#"
            SELECT id, user_id, github_id, github_username, github_avatar_url,
                   access_token_encrypted, access_token_nonce, scopes, is_primary,
                   created_at, updated_at
            FROM linked_github_accounts
            WHERE user_id = $1
            ORDER BY created_at ASC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(account)
    }

    /// Check if a GitHub account is already linked to this user
    pub async fn find_by_github_id(
        pool: &PgPool,
        user_id: Uuid,
        github_id: i64,
    ) -> Result<Option<LinkedGitHubAccount>> {
        let account = sqlx::query_as::<_, LinkedGitHubAccount>(
            r#"
            SELECT id, user_id, github_id, github_username, github_avatar_url,
                   scopes, is_primary, created_at, updated_at
            FROM linked_github_accounts
            WHERE user_id = $1 AND github_id = $2
            "#,
        )
        .bind(user_id)
        .bind(github_id)
        .fetch_optional(pool)
        .await?;

        Ok(account)
    }

    /// Create a new linked GitHub account
    pub async fn create(
        pool: &PgPool,
        data: CreateLinkedGitHubAccount,
    ) -> Result<LinkedGitHubAccount> {
        // Check if this is the first account for the user (make it primary)
        let existing_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM linked_github_accounts WHERE user_id = $1",
        )
        .bind(data.user_id)
        .fetch_one(pool)
        .await?;

        let is_primary = existing_count.0 == 0;

        let account = sqlx::query_as::<_, LinkedGitHubAccount>(
            r#"
            INSERT INTO linked_github_accounts
                (user_id, github_id, github_username, github_avatar_url,
                 access_token_encrypted, access_token_nonce, scopes, is_primary)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, user_id, github_id, github_username, github_avatar_url,
                      scopes, is_primary, created_at, updated_at
            "#,
        )
        .bind(data.user_id)
        .bind(data.github_id)
        .bind(&data.github_username)
        .bind(&data.github_avatar_url)
        .bind(&data.access_token_encrypted)
        .bind(&data.access_token_nonce)
        .bind(&data.scopes)
        .bind(is_primary)
        .fetch_one(pool)
        .await?;

        Ok(account)
    }

    /// Update token for an existing linked account
    pub async fn update_token(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        access_token_encrypted: &[u8],
        access_token_nonce: &[u8],
        scopes: Option<&str>,
    ) -> Result<LinkedGitHubAccount> {
        let account = sqlx::query_as::<_, LinkedGitHubAccount>(
            r#"
            UPDATE linked_github_accounts
            SET access_token_encrypted = $3,
                access_token_nonce = $4,
                scopes = COALESCE($5, scopes),
                updated_at = NOW()
            WHERE id = $1 AND user_id = $2
            RETURNING id, user_id, github_id, github_username, github_avatar_url,
                      scopes, is_primary, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(access_token_encrypted)
        .bind(access_token_nonce)
        .bind(scopes)
        .fetch_one(pool)
        .await?;

        Ok(account)
    }

    /// Delete a linked GitHub account
    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM linked_github_accounts WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Set an account as primary (and unset all others)
    pub async fn set_primary(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool> {
        // Start a transaction to ensure atomicity
        let mut tx = pool.begin().await?;

        // Unset all primary flags for this user
        sqlx::query(
            "UPDATE linked_github_accounts SET is_primary = false WHERE user_id = $1",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        // Set the specified account as primary
        let result = sqlx::query(
            "UPDATE linked_github_accounts SET is_primary = true WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get count of linked accounts for a user
    pub async fn count_by_user(pool: &PgPool, user_id: Uuid) -> Result<i64> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM linked_github_accounts WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(count.0)
    }
}
