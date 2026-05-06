-- Add missing primary regions for existing servers
-- This fixes servers that were created before the automatic primary region creation was added

INSERT INTO server_regions (server_id, region, is_primary, status)
SELECT id, region, true,
    CASE
        WHEN status = 'running' THEN 'running'
        ELSE 'pending'
    END
FROM mcp_servers
WHERE id NOT IN (SELECT server_id FROM server_regions WHERE is_primary = true)
ON CONFLICT (server_id, region) DO NOTHING;
