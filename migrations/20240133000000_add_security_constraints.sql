-- Security and integrity improvements
-- Based on schema review (修正.md)

-- ============================================================================
-- #1 Stripe event deduplication (CRITICAL)
-- Stripe webhooks can be delivered multiple times, UNIQUE prevents double processing
-- ============================================================================

-- billing_events.stripe_event_id - prevent duplicate event processing
-- Using WHERE clause to allow multiple NULL values (events without stripe_event_id)
CREATE UNIQUE INDEX IF NOT EXISTS unq_billing_events_stripe_event_id
ON billing_events(stripe_event_id)
WHERE stripe_event_id IS NOT NULL;

-- payments.stripe_invoice_id - prevent duplicate invoice processing
ALTER TABLE payments
ADD CONSTRAINT unq_payments_stripe_invoice_id UNIQUE (stripe_invoice_id);

-- ============================================================================
-- #14 Missing FK indexes (PostgreSQL doesn't auto-create these)
-- ============================================================================

-- oauth_authorization_codes.user_id - for user session lookups
CREATE INDEX IF NOT EXISTS idx_oauth_codes_user_id
ON oauth_authorization_codes(user_id);

-- deployments.deployed_by - for user deployment history
CREATE INDEX IF NOT EXISTS idx_deployments_deployed_by
ON deployments(deployed_by)
WHERE deployed_by IS NOT NULL;

-- server_regions.is_primary - for quick primary region lookup
CREATE INDEX IF NOT EXISTS idx_server_regions_primary
ON server_regions(server_id)
WHERE is_primary = true;

-- ============================================================================
-- #15 error_hints.keywords GIN index for array matching
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_error_hints_keywords
ON error_hints USING GIN (keywords);

-- ============================================================================
-- #17 JSONB scopes validation
-- Ensure scopes is always a JSON array, never null or object
-- ============================================================================

-- api_keys.scopes must be a JSON array
ALTER TABLE api_keys
ADD CONSTRAINT chk_api_keys_scopes_is_array
CHECK (jsonb_typeof(scopes) = 'array');

-- oauth_clients.scopes must be a JSON array
ALTER TABLE oauth_clients
ADD CONSTRAINT chk_oauth_clients_scopes_is_array
CHECK (jsonb_typeof(scopes) = 'array');

-- oauth_authorization_codes.scopes must be a JSON array
ALTER TABLE oauth_authorization_codes
ADD CONSTRAINT chk_oauth_codes_scopes_is_array
CHECK (jsonb_typeof(scopes) = 'array');

-- oauth_access_tokens.scopes must be a JSON array
ALTER TABLE oauth_access_tokens
ADD CONSTRAINT chk_oauth_access_tokens_scopes_is_array
CHECK (jsonb_typeof(scopes) = 'array');

-- oauth_refresh_tokens.scopes must be a JSON array
ALTER TABLE oauth_refresh_tokens
ADD CONSTRAINT chk_oauth_refresh_tokens_scopes_is_array
CHECK (jsonb_typeof(scopes) = 'array');

-- ============================================================================
-- Additional safety indexes
-- ============================================================================

-- api_keys.workspace_id + server_id for scoped key lookups
CREATE INDEX IF NOT EXISTS idx_api_keys_workspace_server
ON api_keys(workspace_id, server_id);

-- request_logs partitioning preparation note:
-- TODO: For high-volume production, consider:
-- 1. Partitioning request_logs by created_at (monthly)
-- 2. Moving build_logs to object storage (S3/R2)
-- 3. Implementing log retention policy with DROP PARTITION
