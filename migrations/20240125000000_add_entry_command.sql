-- Add entry_command column to mcp_servers
-- Custom command to start the MCP server (e.g., "python server.py", "uv run mcp-server")
-- If NULL, auto-detect based on project structure

ALTER TABLE mcp_servers
ADD COLUMN entry_command VARCHAR(500) NULL;

COMMENT ON COLUMN mcp_servers.entry_command IS 'Custom entry command for the MCP server. If NULL, auto-detect based on project structure.';
