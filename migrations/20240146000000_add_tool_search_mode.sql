-- Per-server "search-first" tool mode for the proxy.
--
-- When true (opt-in, default false), the proxy collapses the server's `tools/list`
-- into just two meta-tools — `search_tools` and `call_tool` — instead of exposing the
-- full tool catalog. The AI client then discovers tools on demand via `search_tools`
-- (served from the catalog in the `tools` table) and invokes them via `call_tool`,
-- so the upfront tool-schema token cost stays roughly constant regardless of how many
-- tools the server actually has. Off by default because it changes the tool surface
-- the client sees, which not all clients expect.
ALTER TABLE mcp_servers
    ADD COLUMN tool_search_mode BOOLEAN NOT NULL DEFAULT false;
