-- Add auth_enabled column to mcp_servers table
-- When false, NodeFlare's proxy will skip authentication and forward requests directly
-- Default is true to maintain backwards compatibility with existing servers

ALTER TABLE mcp_servers
ADD COLUMN auth_enabled BOOLEAN NOT NULL DEFAULT true;

-- Add comment for documentation
COMMENT ON COLUMN mcp_servers.auth_enabled IS 'When false, skip NodeFlare authentication layer (for servers that handle their own auth)';
