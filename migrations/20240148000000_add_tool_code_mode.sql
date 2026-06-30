-- Per-server "code execution" tool mode (opt-in, default false).
--
-- When true, the proxy exposes a `run_code` tool (plus `search_tools` for discovery)
-- instead of the raw tool list. The AI writes JavaScript against a typed API generated
-- from the tool catalog; the proxy runs that code in a locked-down Deno sandbox (a
-- dedicated Fly app = Firecracker microVM, no fs/env, egress restricted to the proxy's
-- internal tool-call endpoint) and returns only the final result. This keeps both
-- tool-schema and intermediate-result tokens low for multi-step tasks.
--
-- Requires PROXY_CODE_RUNNER_URL to be configured on the proxy; otherwise code mode is
-- disabled at runtime and the server behaves as if this flag were false.
ALTER TABLE mcp_servers
    ADD COLUMN tool_code_mode BOOLEAN NOT NULL DEFAULT false;
