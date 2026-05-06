-- Add cancel_at column to server_regions for period-end cancellation
ALTER TABLE server_regions ADD COLUMN IF NOT EXISTS cancel_at TIMESTAMPTZ;

-- Index for finding regions pending cancellation
CREATE INDEX IF NOT EXISTS idx_server_regions_cancel_at ON server_regions(cancel_at) WHERE cancel_at IS NOT NULL;
