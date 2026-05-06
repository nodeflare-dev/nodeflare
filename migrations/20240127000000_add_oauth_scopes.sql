-- Add scopes column to oauth_clients for permission control
-- This allows OAuth clients to have restricted permissions like access tokens

ALTER TABLE oauth_clients
ADD COLUMN scopes JSONB NOT NULL DEFAULT '["*"]';

-- Add comment explaining the column
COMMENT ON COLUMN oauth_clients.scopes IS 'Allowed scopes for this OAuth client. Default is full access (["*"])';
