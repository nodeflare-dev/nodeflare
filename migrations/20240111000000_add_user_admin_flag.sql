-- Add is_admin flag to users table for system-wide admin permissions
ALTER TABLE users ADD COLUMN is_admin BOOLEAN NOT NULL DEFAULT FALSE;

-- Create index for admin lookups
CREATE INDEX idx_users_is_admin ON users(is_admin) WHERE is_admin = TRUE;
