-- Drop unused usage_records table
-- This table was created for billing but never implemented
-- Actual billing uses server_regions.stripe_usage_record_id instead

DROP INDEX IF EXISTS idx_usage_records_workspace;
DROP TABLE IF EXISTS usage_records;
