use crate::models::{
    CreateOAuthAccessToken, CreateOAuthAuthorizationCode, CreateOAuthClient,
    CreateOAuthRefreshToken, OAuthAccessToken, OAuthAccessTokenWithClient,
    OAuthAuthorizationCode, OAuthClient, OAuthRefreshToken,
};
use mcp_common::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct OAuthClientRepository;

impl OAuthClientRepository {
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<OAuthClient>> {
        let client = sqlx::query_as::<_, OAuthClient>(
            r#"
            SELECT id, client_id, client_secret_hash, client_name, redirect_uris,
                   grant_types, token_endpoint_auth_method, workspace_id, server_id,
                   scopes, is_active, is_dynamic, software_id, software_version,
                   access_token_ttl_seconds, created_at, updated_at
            FROM oauth_clients
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(client)
    }

    pub async fn find_by_client_id(pool: &PgPool, client_id: &str) -> Result<Option<OAuthClient>> {
        let client = sqlx::query_as::<_, OAuthClient>(
            r#"
            SELECT id, client_id, client_secret_hash, client_name, redirect_uris,
                   grant_types, token_endpoint_auth_method, workspace_id, server_id,
                   scopes, is_active, is_dynamic, software_id, software_version,
                   access_token_ttl_seconds, created_at, updated_at
            FROM oauth_clients
            WHERE client_id = $1
            "#,
        )
        .bind(client_id)
        .fetch_optional(pool)
        .await?;

        Ok(client)
    }

    pub async fn find_by_server_id(pool: &PgPool, server_id: Uuid) -> Result<Option<OAuthClient>> {
        let client = sqlx::query_as::<_, OAuthClient>(
            r#"
            SELECT id, client_id, client_secret_hash, client_name, redirect_uris,
                   grant_types, token_endpoint_auth_method, workspace_id, server_id,
                   scopes, is_active, is_dynamic, software_id, software_version,
                   access_token_ttl_seconds, created_at, updated_at
            FROM oauth_clients
            WHERE server_id = $1
            "#,
        )
        .bind(server_id)
        .fetch_optional(pool)
        .await?;

        Ok(client)
    }

    pub async fn list_by_workspace(pool: &PgPool, workspace_id: Uuid) -> Result<Vec<OAuthClient>> {
        let clients = sqlx::query_as::<_, OAuthClient>(
            r#"
            SELECT id, client_id, client_secret_hash, client_name, redirect_uris,
                   grant_types, token_endpoint_auth_method, workspace_id, server_id,
                   scopes, is_active, is_dynamic, software_id, software_version,
                   access_token_ttl_seconds, created_at, updated_at
            FROM oauth_clients
            WHERE workspace_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;

        Ok(clients)
    }

    pub async fn create(pool: &PgPool, data: CreateOAuthClient) -> Result<OAuthClient> {
        let redirect_uris_json = serde_json::to_value(&data.redirect_uris)?;
        let grant_types_json = serde_json::to_value(&data.grant_types)?;
        let scopes_json = serde_json::to_value(&data.scopes)?;

        let client = sqlx::query_as::<_, OAuthClient>(
            r#"
            INSERT INTO oauth_clients (client_id, client_secret_hash, client_name, redirect_uris,
                                       grant_types, token_endpoint_auth_method, workspace_id, server_id,
                                       scopes, is_dynamic, software_id, software_version,
                                       access_token_ttl_seconds)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id, client_id, client_secret_hash, client_name, redirect_uris,
                      grant_types, token_endpoint_auth_method, workspace_id, server_id,
                      scopes, is_active, is_dynamic, software_id, software_version,
                      access_token_ttl_seconds, created_at, updated_at
            "#,
        )
        .bind(&data.client_id)
        .bind(&data.client_secret_hash)
        .bind(&data.client_name)
        .bind(redirect_uris_json)
        .bind(grant_types_json)
        .bind(&data.token_endpoint_auth_method)
        .bind(data.workspace_id)
        .bind(data.server_id)
        .bind(scopes_json)
        .bind(data.is_dynamic)
        .bind(&data.software_id)
        .bind(&data.software_version)
        .bind(data.access_token_ttl_seconds)
        .fetch_one(pool)
        .await?;

        Ok(client)
    }

    pub async fn update_secret(
        pool: &PgPool,
        id: Uuid,
        client_secret_hash: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE oauth_clients SET client_secret_hash = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(client_secret_hash)
        .bind(id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM oauth_clients WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }
}

pub struct OAuthAuthorizationCodeRepository;

impl OAuthAuthorizationCodeRepository {
    pub async fn find_by_code_hash(
        pool: &PgPool,
        code_hash: &str,
    ) -> Result<Option<OAuthAuthorizationCode>> {
        let code = sqlx::query_as::<_, OAuthAuthorizationCode>(
            r#"
            SELECT id, code_hash, client_id, user_id, redirect_uri, scopes,
                   code_challenge, code_challenge_method, expires_at, used_at, created_at
            FROM oauth_authorization_codes
            WHERE code_hash = $1
            "#,
        )
        .bind(code_hash)
        .fetch_optional(pool)
        .await?;

        Ok(code)
    }

    pub async fn create(
        pool: &PgPool,
        data: CreateOAuthAuthorizationCode,
    ) -> Result<OAuthAuthorizationCode> {
        let scopes_json = serde_json::to_value(&data.scopes)?;

        let code = sqlx::query_as::<_, OAuthAuthorizationCode>(
            r#"
            INSERT INTO oauth_authorization_codes (code_hash, client_id, user_id, redirect_uri,
                                                   scopes, code_challenge, code_challenge_method, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, code_hash, client_id, user_id, redirect_uri, scopes,
                      code_challenge, code_challenge_method, expires_at, used_at, created_at
            "#,
        )
        .bind(&data.code_hash)
        .bind(data.client_id)
        .bind(data.user_id)
        .bind(&data.redirect_uri)
        .bind(scopes_json)
        .bind(&data.code_challenge)
        .bind(&data.code_challenge_method)
        .bind(data.expires_at)
        .fetch_one(pool)
        .await?;

        Ok(code)
    }

    pub async fn mark_as_used(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE oauth_authorization_codes SET used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn delete_expired(pool: &PgPool) -> Result<u64> {
        let result =
            sqlx::query("DELETE FROM oauth_authorization_codes WHERE expires_at < NOW()")
                .execute(pool)
                .await?;

        Ok(result.rows_affected())
    }
}

pub struct OAuthAccessTokenRepository;

impl OAuthAccessTokenRepository {
    pub async fn find_by_token_hash(
        pool: &PgPool,
        token_hash: &str,
    ) -> Result<Option<OAuthAccessToken>> {
        let token = sqlx::query_as::<_, OAuthAccessToken>(
            r#"
            SELECT id, token_hash, client_id, user_id, scopes,
                   expires_at, revoked_at, last_used_at, created_at
            FROM oauth_access_tokens
            WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await?;

        Ok(token)
    }

    pub async fn find_by_token_hash_with_client(
        pool: &PgPool,
        token_hash: &str,
    ) -> Result<Option<OAuthAccessTokenWithClient>> {
        let token = sqlx::query_as::<_, OAuthAccessTokenWithClient>(
            r#"
            SELECT t.id, t.token_hash, t.client_id, t.user_id, t.scopes,
                   t.expires_at, t.revoked_at, t.last_used_at, t.created_at,
                   c.client_id as oauth_client_id, c.workspace_id, c.server_id,
                   c.is_active as client_is_active
            FROM oauth_access_tokens t
            JOIN oauth_clients c ON t.client_id = c.id
            WHERE t.token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await?;

        Ok(token)
    }

    pub async fn create(pool: &PgPool, data: CreateOAuthAccessToken) -> Result<OAuthAccessToken> {
        let scopes_json = serde_json::to_value(&data.scopes)?;

        let token = sqlx::query_as::<_, OAuthAccessToken>(
            r#"
            INSERT INTO oauth_access_tokens (token_hash, client_id, user_id, scopes, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, token_hash, client_id, user_id, scopes,
                      expires_at, revoked_at, last_used_at, created_at
            "#,
        )
        .bind(&data.token_hash)
        .bind(data.client_id)
        .bind(data.user_id)
        .bind(scopes_json)
        .bind(data.expires_at)
        .fetch_one(pool)
        .await?;

        Ok(token)
    }

    pub async fn revoke(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE oauth_access_tokens SET revoked_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn revoke_by_client(pool: &PgPool, client_id: Uuid) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE oauth_access_tokens SET revoked_at = NOW() WHERE client_id = $1 AND revoked_at IS NULL",
        )
        .bind(client_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn update_last_used(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE oauth_access_tokens SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn delete_expired(pool: &PgPool) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM oauth_access_tokens WHERE expires_at < NOW() AND revoked_at IS NOT NULL",
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }
}

pub struct OAuthRefreshTokenRepository;

impl OAuthRefreshTokenRepository {
    pub async fn find_by_token_hash(
        pool: &PgPool,
        token_hash: &str,
    ) -> Result<Option<OAuthRefreshToken>> {
        let token = sqlx::query_as::<_, OAuthRefreshToken>(
            r#"
            SELECT id, token_hash, client_id, user_id, scopes,
                   access_token_id, expires_at, revoked_at, created_at
            FROM oauth_refresh_tokens
            WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await?;

        Ok(token)
    }

    pub async fn create(
        pool: &PgPool,
        data: CreateOAuthRefreshToken,
    ) -> Result<OAuthRefreshToken> {
        let scopes_json = serde_json::to_value(&data.scopes)?;

        let token = sqlx::query_as::<_, OAuthRefreshToken>(
            r#"
            INSERT INTO oauth_refresh_tokens (token_hash, client_id, user_id, scopes, access_token_id, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, token_hash, client_id, user_id, scopes,
                      access_token_id, expires_at, revoked_at, created_at
            "#,
        )
        .bind(&data.token_hash)
        .bind(data.client_id)
        .bind(data.user_id)
        .bind(scopes_json)
        .bind(data.access_token_id)
        .bind(data.expires_at)
        .fetch_one(pool)
        .await?;

        Ok(token)
    }

    pub async fn revoke(pool: &PgPool, id: Uuid) -> Result<()> {
        sqlx::query("UPDATE oauth_refresh_tokens SET revoked_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn revoke_by_client(pool: &PgPool, client_id: Uuid) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE oauth_refresh_tokens SET revoked_at = NOW() WHERE client_id = $1 AND revoked_at IS NULL",
        )
        .bind(client_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn delete_expired(pool: &PgPool) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM oauth_refresh_tokens WHERE expires_at < NOW() AND revoked_at IS NOT NULL",
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }
}
