-- Deploy webhooks for notifications
CREATE TABLE IF NOT EXISTS deploy_webhooks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    server_id UUID NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    webhook_url TEXT NOT NULL,
    webhook_type VARCHAR(50) NOT NULL DEFAULT 'custom',
    events TEXT[] NOT NULL DEFAULT ARRAY['deploy_success', 'deploy_failure'],
    secret VARCHAR(255),
    is_active BOOLEAN NOT NULL DEFAULT true,
    last_triggered_at TIMESTAMPTZ,
    last_status VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_deploy_webhooks_server_id') THEN
        CREATE INDEX idx_deploy_webhooks_server_id ON deploy_webhooks(server_id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_deploy_webhooks_is_active') THEN
        CREATE INDEX idx_deploy_webhooks_is_active ON deploy_webhooks(is_active) WHERE is_active = true;
    END IF;
END $$;
