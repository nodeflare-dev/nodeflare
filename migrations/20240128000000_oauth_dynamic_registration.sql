-- Support Dynamic Client Registration (RFC 7591)
-- Make workspace_id nullable for dynamically registered public clients

ALTER TABLE oauth_clients ALTER COLUMN workspace_id DROP NOT NULL;

-- Add column to track dynamically registered clients
ALTER TABLE oauth_clients ADD COLUMN is_dynamic BOOLEAN NOT NULL DEFAULT FALSE;

-- Add software_id and software_version for RFC 7591 compliance
ALTER TABLE oauth_clients ADD COLUMN software_id VARCHAR(255);
ALTER TABLE oauth_clients ADD COLUMN software_version VARCHAR(255);

-- Index for dynamic clients
CREATE INDEX idx_oauth_clients_is_dynamic ON oauth_clients(is_dynamic);
