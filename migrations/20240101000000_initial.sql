-- MCP Cloud Initial Schema

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ============================================================================
-- Users
-- ============================================================================
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    github_id BIGINT UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_github_id ON users(github_id);
CREATE INDEX idx_users_email ON users(email);

-- ============================================================================
-- Workspaces
-- ============================================================================
CREATE TABLE workspaces (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(63) UNIQUE NOT NULL,
    plan VARCHAR(20) NOT NULL DEFAULT 'free',
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_workspaces_slug ON workspaces(slug);
CREATE INDEX idx_workspaces_owner ON workspaces(owner_id);

-- ============================================================================
-- Workspace Members
-- ============================================================================
CREATE TABLE workspace_members (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'member',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, user_id)
);

CREATE INDEX idx_workspace_members_user ON workspace_members(user_id);

-- ============================================================================
-- MCP Servers
-- ============================================================================
CREATE TABLE mcp_servers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(63) NOT NULL,
    description TEXT,
    github_repo VARCHAR(255) NOT NULL,
    github_branch VARCHAR(255) NOT NULL DEFAULT 'main',
    github_installation_id BIGINT,
    runtime VARCHAR(20) NOT NULL DEFAULT 'node',
    visibility VARCHAR(20) NOT NULL DEFAULT 'private',
    status VARCHAR(20) NOT NULL DEFAULT 'inactive',
    endpoint_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, slug)
);

CREATE INDEX idx_mcp_servers_workspace ON mcp_servers(workspace_id);
CREATE INDEX idx_mcp_servers_slug ON mcp_servers(slug);
CREATE INDEX idx_mcp_servers_status ON mcp_servers(status);

-- ============================================================================
-- Deployments
-- ============================================================================
CREATE TABLE deployments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    version INT NOT NULL,
    commit_sha VARCHAR(40) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    build_logs TEXT,
    error_message TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    deployed_by UUID REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_deployments_server ON deployments(server_id);
CREATE INDEX idx_deployments_server_version ON deployments(server_id, version DESC);
CREATE INDEX idx_deployments_status ON deployments(status);

-- ============================================================================
-- Tools
-- ============================================================================
CREATE TABLE tools (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    input_schema JSONB,
    enabled BOOLEAN NOT NULL DEFAULT true,
    permission_level VARCHAR(20) NOT NULL DEFAULT 'normal',
    rate_limit_per_minute INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (server_id, name)
);

CREATE INDEX idx_tools_server ON tools(server_id);

-- ============================================================================
-- API Keys
-- ============================================================================
CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    server_id UUID REFERENCES mcp_servers(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    key_prefix VARCHAR(8) NOT NULL,
    key_hash VARCHAR(64) NOT NULL,
    scopes JSONB NOT NULL DEFAULT '[]',
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_api_keys_workspace ON api_keys(workspace_id);
CREATE INDEX idx_api_keys_server ON api_keys(server_id);
CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);

-- ============================================================================
-- Secrets
-- ============================================================================
CREATE TABLE secrets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    key VARCHAR(255) NOT NULL,
    encrypted_value BYTEA NOT NULL,
    nonce BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (server_id, key)
);

CREATE INDEX idx_secrets_server ON secrets(server_id);

-- ============================================================================
-- Request Logs
-- ============================================================================
CREATE TABLE request_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    server_id UUID NOT NULL,
    tool_name VARCHAR(255),
    api_key_id UUID,
    client_info JSONB,
    request_body JSONB,
    response_status VARCHAR(20) NOT NULL,
    error_message TEXT,
    duration_ms INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Partition-ready indexes for request_logs
CREATE INDEX idx_request_logs_server_created ON request_logs(server_id, created_at DESC);
CREATE INDEX idx_request_logs_created ON request_logs(created_at DESC);

-- ============================================================================
-- Usage Records (for billing)
-- ============================================================================
CREATE TABLE usage_records (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    request_count BIGINT NOT NULL DEFAULT 0,
    compute_seconds BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (workspace_id, period_start)
);

CREATE INDEX idx_usage_records_workspace ON usage_records(workspace_id);

-- ============================================================================
-- Refresh Tokens
-- ============================================================================
CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_hash ON refresh_tokens(token_hash);
CREATE INDEX idx_refresh_tokens_expires ON refresh_tokens(expires_at);
