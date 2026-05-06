-- Add root_directory column for monorepo support
ALTER TABLE mcp_servers ADD COLUMN root_directory VARCHAR(255) NOT NULL DEFAULT '';

COMMENT ON COLUMN mcp_servers.root_directory IS 'Subdirectory path within the repository where the MCP server code is located (for monorepo support)';
