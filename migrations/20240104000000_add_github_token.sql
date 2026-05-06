-- Add encrypted GitHub access token to users table
ALTER TABLE users ADD COLUMN github_access_token_encrypted BYTEA;
ALTER TABLE users ADD COLUMN github_access_token_nonce BYTEA;
