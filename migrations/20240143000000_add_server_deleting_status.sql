-- Add a `deleting` server status so deletion can be a soft-delete: the row is marked
-- `deleting`, the Fly app teardown is confirmed, and only THEN is the row hard-deleted.
-- This prevents a permanently orphaned (billable) Fly app when teardown fails after the
-- row was already gone (the old flow hard-deleted before teardown succeeded).
--
-- Relax the existing status CHECK constraint to permit the new value.

ALTER TABLE mcp_servers DROP CONSTRAINT IF EXISTS chk_mcp_servers_status;

ALTER TABLE mcp_servers
ADD CONSTRAINT chk_mcp_servers_status
CHECK (status IN ('inactive', 'building', 'deploying', 'running', 'failed', 'stopped', 'deleting'));
