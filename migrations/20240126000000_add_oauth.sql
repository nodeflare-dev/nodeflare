-- OAuth 2.1 support for MCP authentication
-- Supports Claude custom connectors and other OAuth clients

-- OAuth clients (applications)
CREATE TABLE oauth_clients (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Client ID (UUID format for simplicity)
    client_id VARCHAR(255) NOT NULL UNIQUE,
    -- Client secret (hashed) - NULL for public clients
    client_secret_hash VARCHAR(255),
    -- Human-readable name
    client_name VARCHAR(255) NOT NULL,
    -- Redirect URIs (JSON array)
    redirect_uris JSONB NOT NULL DEFAULT '[]',
    -- Grant types: authorization_code, refresh_token
    grant_types JSONB NOT NULL DEFAULT '["authorization_code", "refresh_token"]',
    -- Token endpoint auth method: none, client_secret_basic, client_secret_post
    token_endpoint_auth_method VARCHAR(50) NOT NULL DEFAULT 'client_secret_basic',
    -- Which workspace owns this client
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    -- Which server this client is for (NULL for all servers in workspace)
    server_id UUID REFERENCES mcp_servers(id) ON DELETE CASCADE,
    -- Is this client active?
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- OAuth authorization codes (short-lived, 10 minutes)
CREATE TABLE oauth_authorization_codes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- The authorization code (hashed)
    code_hash VARCHAR(255) NOT NULL UNIQUE,
    -- Which client requested this code
    client_id UUID NOT NULL REFERENCES oauth_clients(id) ON DELETE CASCADE,
    -- Which user authorized this
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Redirect URI used for this authorization
    redirect_uri VARCHAR(2048) NOT NULL,
    -- Scopes granted (JSON array)
    scopes JSONB NOT NULL DEFAULT '["*"]',
    -- PKCE code challenge (required by MCP spec)
    code_challenge VARCHAR(255) NOT NULL,
    -- PKCE code challenge method (S256)
    code_challenge_method VARCHAR(10) NOT NULL DEFAULT 'S256',
    -- When this code expires
    expires_at TIMESTAMPTZ NOT NULL,
    -- Has this code been used?
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- OAuth access tokens
CREATE TABLE oauth_access_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- The access token (hashed)
    token_hash VARCHAR(255) NOT NULL UNIQUE,
    -- Which client this token was issued to
    client_id UUID NOT NULL REFERENCES oauth_clients(id) ON DELETE CASCADE,
    -- Which user this token represents
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Scopes granted to this token (JSON array)
    scopes JSONB NOT NULL DEFAULT '["*"]',
    -- When this token expires
    expires_at TIMESTAMPTZ NOT NULL,
    -- Has this token been revoked?
    revoked_at TIMESTAMPTZ,
    -- Last used timestamp
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- OAuth refresh tokens
CREATE TABLE oauth_refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- The refresh token (hashed)
    token_hash VARCHAR(255) NOT NULL UNIQUE,
    -- Which client this token was issued to
    client_id UUID NOT NULL REFERENCES oauth_clients(id) ON DELETE CASCADE,
    -- Which user this token represents
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Scopes granted to this token (JSON array)
    scopes JSONB NOT NULL DEFAULT '["*"]',
    -- Related access token
    access_token_id UUID REFERENCES oauth_access_tokens(id) ON DELETE SET NULL,
    -- When this token expires
    expires_at TIMESTAMPTZ NOT NULL,
    -- Has this token been revoked?
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_oauth_clients_workspace ON oauth_clients(workspace_id);
CREATE INDEX idx_oauth_clients_server ON oauth_clients(server_id);
CREATE INDEX idx_oauth_clients_client_id ON oauth_clients(client_id);

CREATE INDEX idx_oauth_codes_code_hash ON oauth_authorization_codes(code_hash);
CREATE INDEX idx_oauth_codes_client_id ON oauth_authorization_codes(client_id);
CREATE INDEX idx_oauth_codes_expires_at ON oauth_authorization_codes(expires_at);

CREATE INDEX idx_oauth_access_tokens_token_hash ON oauth_access_tokens(token_hash);
CREATE INDEX idx_oauth_access_tokens_client_id ON oauth_access_tokens(client_id);
CREATE INDEX idx_oauth_access_tokens_user_id ON oauth_access_tokens(user_id);
CREATE INDEX idx_oauth_access_tokens_expires_at ON oauth_access_tokens(expires_at);

CREATE INDEX idx_oauth_refresh_tokens_token_hash ON oauth_refresh_tokens(token_hash);
CREATE INDEX idx_oauth_refresh_tokens_client_id ON oauth_refresh_tokens(client_id);
CREATE INDEX idx_oauth_refresh_tokens_user_id ON oauth_refresh_tokens(user_id);

-- Function to update updated_at column (if not exists)
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger for updated_at
CREATE TRIGGER update_oauth_clients_updated_at
    BEFORE UPDATE ON oauth_clients
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
