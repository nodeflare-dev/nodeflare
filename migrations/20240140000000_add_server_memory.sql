-- Per-server memory selection. NULL = "auto" (builder falls back to its default /
-- detection floor). Stored in MB; allowed ladder is 256/512/1024/2048, enforced
-- against the workspace plan's ceiling at the API layer.
ALTER TABLE mcp_servers ADD COLUMN memory_mb INTEGER;
