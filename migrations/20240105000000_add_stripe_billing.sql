-- Add Stripe billing fields to workspaces

ALTER TABLE workspaces
ADD COLUMN stripe_customer_id VARCHAR(255),
ADD COLUMN stripe_subscription_id VARCHAR(255),
ADD COLUMN subscription_status VARCHAR(50) DEFAULT 'none',
ADD COLUMN current_period_end TIMESTAMPTZ;

CREATE INDEX idx_workspaces_stripe_customer ON workspaces(stripe_customer_id);
CREATE INDEX idx_workspaces_stripe_subscription ON workspaces(stripe_subscription_id);

-- Billing events log
CREATE TABLE billing_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,
    stripe_event_id VARCHAR(255),
    data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_billing_events_workspace ON billing_events(workspace_id);
CREATE INDEX idx_billing_events_stripe_event ON billing_events(stripe_event_id);
CREATE INDEX idx_billing_events_type ON billing_events(event_type);

-- Payment history
CREATE TABLE payments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    stripe_invoice_id VARCHAR(255) NOT NULL,
    amount_cents INT NOT NULL,
    currency VARCHAR(3) NOT NULL DEFAULT 'usd',
    status VARCHAR(50) NOT NULL,
    paid_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_payments_workspace ON payments(workspace_id);
CREATE INDEX idx_payments_stripe_invoice ON payments(stripe_invoice_id);
