-- Performance indexes for hot path queries
-- These indexes significantly improve proxy request handling performance

-- Index for server lookup by slug with visibility and status filters
-- Used on every proxy request to resolve server by endpoint slug
CREATE INDEX IF NOT EXISTS idx_mcp_servers_slug_visibility_status
ON mcp_servers (slug, visibility, status)
WHERE visibility = 'public' AND status = 'running';

-- Index for API key lookup by hash (critical for authentication)
-- This query runs on every single proxy request
CREATE UNIQUE INDEX IF NOT EXISTS idx_api_keys_key_hash
ON api_keys (key_hash);

-- Composite index for request_logs filtering and pagination
-- Used for analytics queries with server_id and created_at filters
CREATE INDEX IF NOT EXISTS idx_request_logs_server_created
ON request_logs (server_id, created_at DESC);

-- Index for request_logs by status for filtered queries
CREATE INDEX IF NOT EXISTS idx_request_logs_server_status_created
ON request_logs (server_id, response_status, created_at DESC);

-- Index for deployments by server_id and version (for version lookup optimization)
CREATE INDEX IF NOT EXISTS idx_deployments_server_version
ON deployments (server_id, version DESC);

-- Index for workspace members lookup (used in permission checks)
CREATE INDEX IF NOT EXISTS idx_workspace_members_workspace_user
ON workspace_members (workspace_id, user_id);
