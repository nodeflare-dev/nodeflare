-- Multi-region support for MCP servers
-- Allows servers to be deployed to multiple regions with usage-based billing

-- Server regions table - tracks which regions a server is deployed to
CREATE TABLE server_regions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    region VARCHAR(10) NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT false,
    machine_id VARCHAR(255),  -- Fly.io machine ID
    status VARCHAR(20) NOT NULL DEFAULT 'pending',  -- pending, deploying, running, stopped, failed
    endpoint_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (server_id, region)
);

CREATE INDEX idx_server_regions_server ON server_regions(server_id);
CREATE INDEX idx_server_regions_region ON server_regions(region);
CREATE INDEX idx_server_regions_status ON server_regions(status);

-- Region usage tracking for billing (monthly)
CREATE TABLE region_usage (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    region VARCHAR(10) NOT NULL,
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    active_hours INT NOT NULL DEFAULT 0,  -- Hours the region was active
    reported_to_stripe BOOLEAN NOT NULL DEFAULT false,
    stripe_usage_record_id VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (server_id, region, period_start)
);

CREATE INDEX idx_region_usage_workspace ON region_usage(workspace_id);
CREATE INDEX idx_region_usage_server ON region_usage(server_id);
CREATE INDEX idx_region_usage_period ON region_usage(period_start, period_end);
CREATE INDEX idx_region_usage_unreported ON region_usage(reported_to_stripe) WHERE NOT reported_to_stripe;

-- Add stripe subscription item ID for metered billing to workspaces
ALTER TABLE workspaces
ADD COLUMN stripe_region_price_item_id VARCHAR(255);

-- Migrate existing servers: set their current region as primary in server_regions
INSERT INTO server_regions (server_id, region, is_primary, status, endpoint_url)
SELECT id, region, true,
       CASE WHEN status = 'running' THEN 'running' ELSE 'stopped' END,
       endpoint_url
FROM mcp_servers
WHERE region IS NOT NULL AND region != '';
