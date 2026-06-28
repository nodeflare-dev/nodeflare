-- Per-server internal listening port for Streamable HTTP (SSE) servers. NULL = "auto"
-- (builder falls back to the runtime default: node 3000, python 8000, go/rust 8080).
-- Lets users deploy an existing HTTP MCP server that hardcodes its own port instead of
-- reading $PORT. Ignored for stdio transport, which always uses the adapter's port.
ALTER TABLE mcp_servers ADD COLUMN port INTEGER;
