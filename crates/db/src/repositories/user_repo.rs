use crate::models::{
    CreateUserFromEmail, CreateUserFromGitHub, CreateUserFromGoogle,
    EmailVerificationToken, TokenType, UpdateUser, User, UserWithToken,
};
use mcp_common::Result;
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub struct UserRepository;

impl UserRepository {
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, github_id, google_id, email, name, avatar_url, is_admin,
                   email_verified, auth_provider, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    /// Check if a user is a system admin
    pub async fn is_admin(pool: &PgPool, id: Uuid) -> Result<bool> {
        let result: Option<(bool,)> = sqlx::query_as(
            r#"
            SELECT is_admin
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(result.map(|(is_admin,)| is_admin).unwrap_or(false))
    }

    pub async fn find_by_github_id(pool: &PgPool, github_id: i64) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, github_id, google_id, email, name, avatar_url, is_admin,
                   email_verified, auth_provider, created_at, updated_at
            FROM users
            WHERE github_id = $1
            "#,
        )
        .bind(github_id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_google_id(pool: &PgPool, google_id: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, github_id, google_id, email, name, avatar_url, is_admin,
                   email_verified, auth_provider, created_at, updated_at
            FROM users
            WHERE google_id = $1
            "#,
        )
        .bind(google_id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, github_id, google_id, email, name, avatar_url, is_admin,
                   email_verified, auth_provider, created_at, updated_at
            FROM users
            WHERE email = $1
            "#,
        )
        .bind(email)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn create_from_github(pool: &PgPool, data: CreateUserFromGitHub) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (github_id, email, name, avatar_url, email_verified, auth_provider)
            VALUES ($1, $2, $3, $4, true, 'github')
            RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                      email_verified, auth_provider, created_at, updated_at
            "#,
        )
        .bind(data.github_id)
        .bind(&data.email)
        .bind(&data.name)
        .bind(&data.avatar_url)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn create_from_google(pool: &PgPool, data: CreateUserFromGoogle) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (google_id, email, name, avatar_url, email_verified, auth_provider)
            VALUES ($1, $2, $3, $4, true, 'google')
            RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                      email_verified, auth_provider, created_at, updated_at
            "#,
        )
        .bind(&data.google_id)
        .bind(&data.email)
        .bind(&data.name)
        .bind(&data.avatar_url)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn create_from_email(pool: &PgPool, data: CreateUserFromEmail) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (email, name, password_hash, email_verified, auth_provider)
            VALUES ($1, $2, $3, false, 'email')
            RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                      email_verified, auth_provider, created_at, updated_at
            "#,
        )
        .bind(&data.email)
        .bind(&data.name)
        .bind(&data.password_hash)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn update(pool: &PgPool, id: Uuid, data: UpdateUser) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET
                email = COALESCE($2, email),
                name = COALESCE($3, name),
                avatar_url = COALESCE($4, avatar_url),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                      email_verified, auth_provider, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&data.email)
        .bind(&data.name)
        .bind(&data.avatar_url)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn upsert_from_github(
        pool: &PgPool,
        github_id: i64,
        email: &str,
        name: &str,
        avatar_url: Option<&str>,
    ) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (github_id, email, name, avatar_url, email_verified, auth_provider)
            VALUES ($1, $2, $3, $4, true, 'github')
            ON CONFLICT (github_id) DO UPDATE SET
                email = EXCLUDED.email,
                name = EXCLUDED.name,
                avatar_url = EXCLUDED.avatar_url,
                updated_at = NOW()
            RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                      email_verified, auth_provider, created_at, updated_at
            "#,
        )
        .bind(github_id)
        .bind(email)
        .bind(name)
        .bind(avatar_url)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn upsert_from_google(
        pool: &PgPool,
        google_id: &str,
        email: &str,
        name: &str,
        avatar_url: Option<&str>,
    ) -> Result<User> {
        // First, check if user with this google_id already exists
        if let Some(_) = Self::find_by_google_id(pool, google_id).await? {
            // Update existing Google user
            let updated_user = sqlx::query_as::<_, User>(
                r#"
                UPDATE users
                SET email = $2, name = $3, avatar_url = COALESCE($4, avatar_url), updated_at = NOW()
                WHERE google_id = $1
                RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                          email_verified, auth_provider, created_at, updated_at
                "#,
            )
            .bind(google_id)
            .bind(email)
            .bind(name)
            .bind(avatar_url)
            .fetch_one(pool)
            .await?;
            return Ok(updated_user);
        }

        // Check if user with this email exists (registered via email or GitHub)
        if let Some(_) = Self::find_by_email(pool, email).await? {
            // Link Google account to existing user
            let updated_user = sqlx::query_as::<_, User>(
                r#"
                UPDATE users
                SET google_id = $1, email_verified = true, updated_at = NOW()
                WHERE email = $2
                RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                          email_verified, auth_provider, created_at, updated_at
                "#,
            )
            .bind(google_id)
            .bind(email)
            .fetch_one(pool)
            .await?;
            return Ok(updated_user);
        }

        // Create new user
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (google_id, email, name, avatar_url, email_verified, auth_provider)
            VALUES ($1, $2, $3, $4, true, 'google')
            RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                      email_verified, auth_provider, created_at, updated_at
            "#,
        )
        .bind(google_id)
        .bind(email)
        .bind(name)
        .bind(avatar_url)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    /// Link Google account to existing user
    pub async fn link_google_account(pool: &PgPool, user_id: Uuid, google_id: &str) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET google_id = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                      email_verified, auth_provider, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(google_id)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    /// Link GitHub account to existing user
    pub async fn link_github_account(pool: &PgPool, user_id: Uuid, github_id: i64) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET github_id = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                      email_verified, auth_provider, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(github_id)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn update_name(pool: &PgPool, id: Uuid, name: &str) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET name = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                      email_verified, auth_provider, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(name)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn update_password(pool: &PgPool, id: Uuid, password_hash: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET password_hash = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(password_hash)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn verify_email(pool: &PgPool, id: Uuid) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET email_verified = true, updated_at = NOW()
            WHERE id = $1
            RETURNING id, github_id, google_id, email, name, avatar_url, is_admin,
                      email_verified, auth_provider, created_at, updated_at
            "#,
        )
        .bind(id)
        .fetch_one(pool)
        .await?;

        Ok(user)
    }

    pub async fn update_github_token(
        pool: &PgPool,
        id: Uuid,
        encrypted_token: &[u8],
        nonce: &[u8],
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET github_access_token_encrypted = $2, github_access_token_nonce = $3, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(encrypted_token)
        .bind(nonce)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn get_with_token(pool: &PgPool, id: Uuid) -> Result<Option<UserWithToken>> {
        let user = sqlx::query_as::<_, UserWithToken>(
            r#"
            SELECT id, github_id, google_id, email, name, avatar_url, is_admin,
                   email_verified, auth_provider, password_hash,
                   github_access_token_encrypted, github_access_token_nonce, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    /// Get user with password hash for email/password login
    pub async fn get_for_email_login(pool: &PgPool, email: &str) -> Result<Option<UserWithToken>> {
        let user = sqlx::query_as::<_, UserWithToken>(
            r#"
            SELECT id, github_id, google_id, email, name, avatar_url, is_admin,
                   email_verified, auth_provider, password_hash,
                   github_access_token_encrypted, github_access_token_nonce, created_at, updated_at
            FROM users
            WHERE email = $1 AND password_hash IS NOT NULL
            "#,
        )
        .bind(email)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }
}

/// Repository for email verification tokens
pub struct EmailVerificationTokenRepository;

impl EmailVerificationTokenRepository {
    /// Create a new verification token
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        token_hash: &str,
        token_type: TokenType,
        expires_at: DateTime<Utc>,
    ) -> Result<EmailVerificationToken> {
        let token = sqlx::query_as::<_, EmailVerificationToken>(
            r#"
            INSERT INTO email_verification_tokens (user_id, token_hash, token_type, expires_at)
            VALUES ($1, $2, $3, $4)
            RETURNING id, user_id, token_hash, token_type, expires_at, created_at, used_at
            "#,
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(token_type.to_string())
        .bind(expires_at)
        .fetch_one(pool)
        .await?;

        Ok(token)
    }

    /// Find token by hash
    pub async fn find_by_hash(pool: &PgPool, token_hash: &str) -> Result<Option<EmailVerificationToken>> {
        let token = sqlx::query_as::<_, EmailVerificationToken>(
            r#"
            SELECT id, user_id, token_hash, token_type, expires_at, created_at, used_at
            FROM email_verification_tokens
            WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await?;

        Ok(token)
    }

    /// Find valid (unused and not expired) token by hash
    pub async fn find_valid_token(pool: &PgPool, token_hash: &str) -> Result<Option<EmailVerificationToken>> {
        let token = sqlx::query_as::<_, EmailVerificationToken>(
            r#"
            SELECT id, user_id, token_hash, token_type, expires_at, created_at, used_at
            FROM email_verification_tokens
            WHERE token_hash = $1
              AND used_at IS NULL
              AND expires_at > NOW()
            "#,
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await?;

        Ok(token)
    }

    /// Mark token as used
    pub async fn mark_as_used(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE email_verification_tokens
            SET used_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Delete token
    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM email_verification_tokens WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Delete all tokens for a user
    pub async fn delete_all_for_user(pool: &PgPool, user_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM email_verification_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Delete all tokens of a specific type for a user
    pub async fn delete_by_type_for_user(pool: &PgPool, user_id: Uuid, token_type: TokenType) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM email_verification_tokens
            WHERE user_id = $1 AND token_type = $2
            "#,
        )
        .bind(user_id)
        .bind(token_type.to_string())
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Cleanup expired tokens
    pub async fn cleanup_expired(pool: &PgPool) -> Result<u64> {
        let result = sqlx::query("DELETE FROM email_verification_tokens WHERE expires_at < NOW()")
            .execute(pool)
            .await?;

        Ok(result.rows_affected())
    }
}
