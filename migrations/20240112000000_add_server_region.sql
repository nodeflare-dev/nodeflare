-- Add region column to mcp_servers table
-- Default to 'nrt' (Tokyo) for existing servers

ALTER TABLE mcp_servers
ADD COLUMN region VARCHAR(10) NOT NULL DEFAULT 'nrt';

-- Create index for region queries
CREATE INDEX idx_mcp_servers_region ON mcp_servers(region);
