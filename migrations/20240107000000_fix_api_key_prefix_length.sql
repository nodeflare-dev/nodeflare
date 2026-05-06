-- Fix api_keys.key_prefix length
-- The code generates 12 character prefixes (e.g., "mcp_abc12345")
-- but the column was defined as VARCHAR(8)

ALTER TABLE api_keys
ALTER COLUMN key_prefix TYPE VARCHAR(16);
