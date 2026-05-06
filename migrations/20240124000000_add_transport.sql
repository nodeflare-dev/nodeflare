-- Add transport field for STDIO/SSE support
-- transport: 'sse' = server exposes HTTP/SSE endpoint (default)
-- transport: 'stdio' = server uses stdin/stdout, needs adapter

ALTER TABLE mcp_servers
ADD COLUMN transport VARCHAR(20) NOT NULL DEFAULT 'sse';

-- Add constraint for valid transport values
ALTER TABLE mcp_servers
ADD CONSTRAINT chk_mcp_servers_transport
CHECK (transport IN ('sse', 'stdio'));

-- Add comment for documentation
COMMENT ON COLUMN mcp_servers.transport IS 'Transport type: sse (HTTP/SSE endpoint) or stdio (stdin/stdout with adapter)';
