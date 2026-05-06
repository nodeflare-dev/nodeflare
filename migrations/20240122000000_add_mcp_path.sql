-- Add mcp_path column for MCP endpoint path configuration
ALTER TABLE mcp_servers ADD COLUMN mcp_path VARCHAR(255) NOT NULL DEFAULT '/mcp';

COMMENT ON COLUMN mcp_servers.mcp_path IS 'HTTP path where the MCP server listens (e.g., /mcp, /sse, /)';
