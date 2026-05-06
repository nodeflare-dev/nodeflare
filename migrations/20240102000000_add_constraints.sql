-- Add CHECK constraints and improve data integrity

-- ============================================================================
-- Add CHECK constraints for enum-like fields
-- ============================================================================

-- Workspaces plan
ALTER TABLE workspaces
ADD CONSTRAINT chk_workspaces_plan
CHECK (plan IN ('free', 'pro', 'enterprise'));

-- Workspace members role
ALTER TABLE workspace_members
ADD CONSTRAINT chk_workspace_members_role
CHECK (role IN ('owner', 'admin', 'member', 'viewer'));

-- MCP Servers runtime
ALTER TABLE mcp_servers
ADD CONSTRAINT chk_mcp_servers_runtime
CHECK (runtime IN ('node', 'python', 'go', 'rust', 'deno', 'bun'));

-- MCP Servers visibility
ALTER TABLE mcp_servers
ADD CONSTRAINT chk_mcp_servers_visibility
CHECK (visibility IN ('private', 'team', 'public'));

-- MCP Servers status
ALTER TABLE mcp_servers
ADD CONSTRAINT chk_mcp_servers_status
CHECK (status IN ('inactive', 'building', 'deploying', 'running', 'failed', 'stopped'));

-- Deployments status
ALTER TABLE deployments
ADD CONSTRAINT chk_deployments_status
CHECK (status IN ('pending', 'building', 'pushing', 'deploying', 'succeeded', 'failed', 'cancelled'));

-- Tools permission level
ALTER TABLE tools
ADD CONSTRAINT chk_tools_permission_level
CHECK (permission_level IN ('normal', 'elevated', 'dangerous'));

-- ============================================================================
-- Add UNIQUE constraint on api_keys.key_hash
-- ============================================================================
ALTER TABLE api_keys
ADD CONSTRAINT unq_api_keys_key_hash UNIQUE (key_hash);

-- ============================================================================
-- Add NOT NULL and format validation where possible
-- ============================================================================

-- Ensure github_repo follows owner/repo format (basic check)
ALTER TABLE mcp_servers
ADD CONSTRAINT chk_mcp_servers_github_repo
CHECK (github_repo ~ '^[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+$');

-- Ensure slug format (lowercase, alphanumeric with hyphens)
ALTER TABLE workspaces
ADD CONSTRAINT chk_workspaces_slug
CHECK (slug ~ '^[a-z0-9][a-z0-9-]*[a-z0-9]$' OR slug ~ '^[a-z0-9]$');

ALTER TABLE mcp_servers
ADD CONSTRAINT chk_mcp_servers_slug
CHECK (slug ~ '^[a-z0-9][a-z0-9-]*[a-z0-9]$' OR slug ~ '^[a-z0-9]$');

-- ============================================================================
-- Add foreign key for request_logs.server_id (optional, for referential integrity)
-- Note: This may impact performance on high-volume logging
-- Uncomment if referential integrity is more important than write performance
-- ============================================================================
-- ALTER TABLE request_logs
-- ADD CONSTRAINT fk_request_logs_server
-- FOREIGN KEY (server_id) REFERENCES mcp_servers(id) ON DELETE CASCADE;

-- ============================================================================
-- Security: Additional indexes for cleanup operations
-- ============================================================================

-- Note: Partial indexes with NOW() are not allowed (not IMMUTABLE)
-- These regular indexes will help with cleanup queries
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_expires_at
ON refresh_tokens(expires_at);

CREATE INDEX IF NOT EXISTS idx_api_keys_expires_at
ON api_keys(expires_at)
WHERE expires_at IS NOT NULL;
