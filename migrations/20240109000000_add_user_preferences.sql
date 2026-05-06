-- User Preferences for storing user-specific settings like sidebar order

CREATE TABLE user_preferences (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    sidebar_order JSONB NOT NULL DEFAULT '["overview", "servers", "apiKeys", "team", "logs", "billing", "settings"]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
