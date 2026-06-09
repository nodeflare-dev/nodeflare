-- Add build_command column to mcp_servers
-- Custom command to build the MCP server before starting (e.g., "npm run build", "npm run compile", "tsc")
-- If NULL, fall back to the runtime default (Node: `npm run build --if-present`).

ALTER TABLE mcp_servers
ADD COLUMN IF NOT EXISTS build_command VARCHAR(500) NULL;

COMMENT ON COLUMN mcp_servers.build_command IS 'Custom build command run at image-build time. If NULL, use the runtime default (Node: npm run build --if-present).';
