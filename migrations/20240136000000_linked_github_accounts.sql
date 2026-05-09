-- Migration: Add linked_github_accounts table for multiple GitHub account support
-- This allows users to connect GitHub accounts separately from their login method

-- Create the linked_github_accounts table
CREATE TABLE linked_github_accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    github_id BIGINT NOT NULL,
    github_username VARCHAR(39) NOT NULL,
    github_avatar_url TEXT,
    access_token_encrypted BYTEA NOT NULL,
    access_token_nonce BYTEA NOT NULL,
    scopes TEXT,
    is_primary BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Same user can't link same GitHub account twice
    UNIQUE (user_id, github_id)
);

-- Indexes for efficient queries
CREATE INDEX idx_linked_github_accounts_user_id ON linked_github_accounts(user_id);
CREATE INDEX idx_linked_github_accounts_github_id ON linked_github_accounts(github_id);

-- Migrate existing GitHub tokens from users table to linked_github_accounts
-- Only for users who have both github_id and github_access_token_encrypted
INSERT INTO linked_github_accounts (
    user_id,
    github_id,
    github_username,
    github_avatar_url,
    access_token_encrypted,
    access_token_nonce,
    is_primary
)
SELECT
    id,
    github_id,
    name,
    avatar_url,
    github_access_token_encrypted,
    github_access_token_nonce,
    true
FROM users
WHERE github_id IS NOT NULL
  AND github_access_token_encrypted IS NOT NULL
  AND github_access_token_nonce IS NOT NULL;

-- Add trigger to update updated_at
CREATE OR REPLACE FUNCTION update_linked_github_accounts_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_linked_github_accounts_updated_at
    BEFORE UPDATE ON linked_github_accounts
    FOR EACH ROW
    EXECUTE FUNCTION update_linked_github_accounts_updated_at();
