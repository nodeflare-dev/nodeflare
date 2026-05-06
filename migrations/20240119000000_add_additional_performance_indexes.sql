-- Additional performance indexes for database optimization
-- Based on query pattern analysis and hot path identification

-- =============================================================================
-- PRIORITY 1: Immediate optimizations (hot paths)
-- =============================================================================

-- Partial index for active subscriptions (billing lookups)
-- Used in workspace plan enforcement checks
CREATE INDEX IF NOT EXISTS idx_workspaces_active_subscriptions
ON workspaces (subscription_status)
WHERE subscription_status IN ('active', 'trialing', 'past_due');

-- Note: idx_request_logs_created already exists in initial migration
-- Speeds up delete_old_logs() which runs periodically

-- Note: idx_users_is_admin already exists in 20240111 migration

-- =============================================================================
-- PRIORITY 2: Medium-term optimizations (secondary hot paths)
-- =============================================================================

-- Multi-region indexes for server_regions queries
CREATE INDEX IF NOT EXISTS idx_server_regions_server_status
ON server_regions (server_id, status);

-- Index for region_usage billing period lookups
CREATE INDEX IF NOT EXISTS idx_region_usage_workspace_period
ON region_usage (workspace_id, period_start DESC, period_end DESC);

-- Composite index for announcements listing (active + date ordering)
CREATE INDEX IF NOT EXISTS idx_announcements_active_published
ON announcements (is_active, published_at DESC);

-- Index for recent users (admin dashboard, user management)
CREATE INDEX IF NOT EXISTS idx_users_created_at
ON users (created_at DESC);

-- =============================================================================
-- PRIORITY 3: Additional optimizations
-- =============================================================================

-- Index for servers by workspace (commonly filtered by workspace_id)
-- Already exists as idx_mcp_servers_workspace but ensure it's DESC for recent-first
CREATE INDEX IF NOT EXISTS idx_mcp_servers_workspace_created
ON mcp_servers (workspace_id, created_at DESC);

-- Index for deployments by status (useful for deployment status queries)
CREATE INDEX IF NOT EXISTS idx_deployments_status_started
ON deployments (status, started_at DESC);

-- Index for tools by server with ordering
CREATE INDEX IF NOT EXISTS idx_tools_server_name
ON tools (server_id, name);

-- Index for secrets by server and key (lookup optimization)
CREATE INDEX IF NOT EXISTS idx_secrets_server_key
ON secrets (server_id, key);

-- Index for API keys expiration cleanup
-- Partial index for non-null expires_at values
CREATE INDEX IF NOT EXISTS idx_api_keys_expires_at_active
ON api_keys (expires_at)
WHERE expires_at IS NOT NULL;

-- Index for deploy_webhooks by server (webhook trigger queries)
CREATE INDEX IF NOT EXISTS idx_deploy_webhooks_server_active
ON deploy_webhooks (server_id, is_active)
WHERE is_active = true;

-- Index for contact_messages (created_at ordering for admin review)
CREATE INDEX IF NOT EXISTS idx_contact_messages_created_desc
ON contact_messages (created_at DESC);

-- =============================================================================
-- ANALYZE updated tables for query planner
-- =============================================================================
ANALYZE workspaces;
ANALYZE request_logs;
ANALYZE users;
ANALYZE server_regions;
ANALYZE region_usage;
ANALYZE announcements;
ANALYZE mcp_servers;
ANALYZE deployments;
ANALYZE tools;
ANALYZE secrets;
ANALYZE api_keys;
ANALYZE deploy_webhooks;
ANALYZE contact_messages;
