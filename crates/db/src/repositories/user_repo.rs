use crate::models::{CreateUser, UpdateUser, User, UserWithToken};
use mcp_common::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct UserRepository;

impl UserRepository {
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, github_id, email, name, avatar_url, is_admin, created_at, updated_at
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
            SELECT id, github_id, email, name, avatar_url, is_admin, created_at, updated_at
            FROM users
            WHERE github_id = $1
            "#,
        )
        .bind(github_id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, github_id, email, name, avatar_url, is_admin, created_at, updated_at
            FROM users
            WHERE email = $1
            "#,
        )
        .bind(email)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }

    pub async fn create(pool: &PgPool, data: CreateUser) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (github_id, email, name, avatar_url)
            VALUES ($1, $2, $3, $4)
            RETURNING id, github_id, email, name, avatar_url, is_admin, created_at, updated_at
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
            RETURNING id, github_id, email, name, avatar_url, is_admin, created_at, updated_at
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
            INSERT INTO users (github_id, email, name, avatar_url)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (github_id) DO UPDATE SET
                email = EXCLUDED.email,
                name = EXCLUDED.name,
                avatar_url = EXCLUDED.avatar_url,
                updated_at = NOW()
            RETURNING id, github_id, email, name, avatar_url, is_admin, created_at, updated_at
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
            RETURNING id, github_id, email, name, avatar_url, is_admin, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(name)
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
            SELECT id, github_id, email, name, avatar_url, is_admin, github_access_token_encrypted, github_access_token_nonce, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(user)
    }
}
