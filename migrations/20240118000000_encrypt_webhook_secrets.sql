-- Encrypt webhook secrets
-- Add encrypted columns for webhook secrets
ALTER TABLE deploy_webhooks
    ADD COLUMN IF NOT EXISTS encrypted_secret BYTEA,
    ADD COLUMN IF NOT EXISTS secret_nonce BYTEA;

-- Note: After running this migration, existing plain-text secrets need to be migrated manually
-- or through an application-level migration script that:
-- 1. Reads the old 'secret' column
-- 2. Encrypts it using the CryptoService
-- 3. Stores in encrypted_secret and secret_nonce
-- 4. Clears the old 'secret' column

-- For new deployments, the 'secret' column can be dropped after migration is complete
-- ALTER TABLE deploy_webhooks DROP COLUMN secret;

COMMENT ON COLUMN deploy_webhooks.encrypted_secret IS 'AES-256-GCM encrypted webhook secret';
COMMENT ON COLUMN deploy_webhooks.secret_nonce IS 'Nonce used for AES-256-GCM encryption';
