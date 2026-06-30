-- Per-server token-optimization flags for the proxy's tools/list handling.
--
-- tool_list_filter_by_scope: when true (default), the proxy removes from a
-- `tools/list` response any tool the calling credential is not allowed to call
-- (NodeFlare-auth mode only). This enforces least privilege at discovery time and
-- cuts the tool-schema tokens an AI client loads upfront. The call-time scope check
-- still applies regardless; this only changes what the client *sees* listed.
--
-- tool_schema_slim: when true (opt-in, default false), the proxy trims verbose tool
-- schemas (long descriptions) before returning the list, to further reduce tokens.
-- Off by default because it alters tool descriptions, which some clients rely on.
ALTER TABLE mcp_servers
    ADD COLUMN tool_list_filter_by_scope BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN tool_schema_slim BOOLEAN NOT NULL DEFAULT false;
