-- Persist the Fly.io app name per server instead of recomputing it from the UUID prefix.
--
-- The old scheme `mcp-<first-uuid-segment>` used only the first 8 hex chars (32 bits) of
-- the UUID, so distinct servers could collide onto the SAME Fly app — leading to
-- cross-tenant overwrite-on-deploy and cross-tenant deletion via DestroyJob.
--
-- We persist the name so it is decided ONCE at creation. Existing rows are backfilled to
-- their CURRENT (old-scheme) name so their already-live Fly apps keep working; only NEW
-- servers get the collision-free full-UUID name (`mcp-<uuid-no-dashes>`).

ALTER TABLE mcp_servers ADD COLUMN fly_app_name TEXT;

-- Backfill existing rows to the name their live Fly app already uses (old scheme).
UPDATE mcp_servers
SET fly_app_name = 'mcp-' || split_part(id::text, '-', 1)
WHERE fly_app_name IS NULL;

ALTER TABLE mcp_servers ALTER COLUMN fly_app_name SET NOT NULL;

-- Non-unique index for lookups. NOT unique on purpose: legacy rows may already collide
-- under the old scheme, and a UNIQUE constraint would fail this migration. New names are
-- collision-free by construction (full UUID).
CREATE INDEX IF NOT EXISTS idx_mcp_servers_fly_app_name ON mcp_servers (fly_app_name);
