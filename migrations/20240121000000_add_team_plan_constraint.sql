-- Add 'team' to the workspaces plan check constraint

-- Drop the old constraint
ALTER TABLE workspaces
DROP CONSTRAINT IF EXISTS chk_workspaces_plan;

-- Add the new constraint with 'team' included
ALTER TABLE workspaces
ADD CONSTRAINT chk_workspaces_plan
CHECK (plan IN ('free', 'pro', 'team', 'enterprise'));
