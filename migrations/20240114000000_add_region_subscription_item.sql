-- Add Stripe region subscription item tracking to workspaces
-- This stores the subscription item ID for additional region billing

ALTER TABLE workspaces
ADD COLUMN stripe_region_subscription_item_id VARCHAR(255);

-- Add comment for documentation
COMMENT ON COLUMN workspaces.stripe_region_subscription_item_id IS 'Stripe subscription item ID for additional region billing (¥300/month per region)';
