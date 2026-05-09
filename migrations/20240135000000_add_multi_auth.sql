-- Multi-Auth Support Migration
-- Adds Google OAuth and Email/Password authentication support

-- ============================================================================
-- Step 1: Make github_id nullable to allow non-GitHub users
-- ============================================================================
ALTER TABLE users ALTER COLUMN github_id DROP NOT NULL;

-- ============================================================================
-- Step 2: Add new authentication columns
-- ============================================================================
-- Google OAuth ID
ALTER TABLE users ADD COLUMN google_id VARCHAR(255) UNIQUE;

-- Password hash for email/password authentication (Argon2)
ALTER TABLE users ADD COLUMN password_hash VARCHAR(255);

-- Email verification status
ALTER TABLE users ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT false;

-- Authentication provider that was used for registration
-- Values: 'github', 'google', 'email'
ALTER TABLE users ADD COLUMN auth_provider VARCHAR(20) NOT NULL DEFAULT 'github';

-- ============================================================================
-- Step 3: Backfill existing users
-- ============================================================================
-- Mark existing GitHub users as email verified (they have verified emails from GitHub)
UPDATE users SET email_verified = true WHERE github_id IS NOT NULL;

-- ============================================================================
-- Step 4: Add constraint to ensure at least one auth method exists
-- ============================================================================
ALTER TABLE users ADD CONSTRAINT users_has_auth_method
    CHECK (github_id IS NOT NULL OR google_id IS NOT NULL OR password_hash IS NOT NULL);

-- ============================================================================
-- Step 5: Create indexes for new columns
-- ============================================================================
CREATE INDEX idx_users_google_id ON users(google_id) WHERE google_id IS NOT NULL;
CREATE INDEX idx_users_auth_provider ON users(auth_provider);

-- ============================================================================
-- Step 6: Create email verification tokens table
-- ============================================================================
CREATE TABLE email_verification_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(64) NOT NULL UNIQUE,
    token_type VARCHAR(20) NOT NULL, -- 'verification' or 'password_reset'
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    used_at TIMESTAMPTZ -- Track when token was used (for audit)
);

CREATE INDEX idx_email_verification_tokens_user ON email_verification_tokens(user_id);
CREATE INDEX idx_email_verification_tokens_hash ON email_verification_tokens(token_hash);
CREATE INDEX idx_email_verification_tokens_expires ON email_verification_tokens(expires_at);
CREATE INDEX idx_email_verification_tokens_type ON email_verification_tokens(token_type);

-- ============================================================================
-- Step 7: Add comment for documentation
-- ============================================================================
COMMENT ON COLUMN users.github_id IS 'GitHub user ID (nullable for non-GitHub users)';
COMMENT ON COLUMN users.google_id IS 'Google OAuth subject ID (sub claim)';
COMMENT ON COLUMN users.password_hash IS 'Argon2 hashed password for email/password auth';
COMMENT ON COLUMN users.email_verified IS 'Whether the email address has been verified';
COMMENT ON COLUMN users.auth_provider IS 'Primary auth provider used for registration: github, google, or email';
COMMENT ON TABLE email_verification_tokens IS 'Tokens for email verification and password reset';
