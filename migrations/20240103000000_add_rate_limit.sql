-- Add rate_limit_per_minute to mcp_servers table
ALTER TABLE mcp_servers
ADD COLUMN rate_limit_per_minute INT DEFAULT 100;

-- Add comment
COMMENT ON COLUMN mcp_servers.rate_limit_per_minute IS 'Rate limit in requests per minute, NULL means use workspace default';
