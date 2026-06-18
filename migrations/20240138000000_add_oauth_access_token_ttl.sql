-- Per-OAuth-client access token TTL.
-- NULL  = no expiration (issued access tokens never expire)
-- > 0   = access token lifetime in seconds
ALTER TABLE oauth_clients ADD COLUMN access_token_ttl_seconds BIGINT;

-- Backfill existing clients (incl. Claude's auto-registered DCR clients) to 30 days
-- so day-old sessions stop being rejected as expired.
UPDATE oauth_clients SET access_token_ttl_seconds = 2592000 WHERE access_token_ttl_seconds IS NULL;

COMMENT ON COLUMN oauth_clients.access_token_ttl_seconds IS
  'Access token lifetime in seconds for tokens issued to this client. NULL = no expiration.';
