-- User notification settings
CREATE TABLE IF NOT EXISTS notification_settings (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    email_deploy_success BOOLEAN NOT NULL DEFAULT true,
    email_deploy_failure BOOLEAN NOT NULL DEFAULT true,
    email_server_down BOOLEAN NOT NULL DEFAULT true,
    email_weekly_report BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
