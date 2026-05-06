-- Announcements table for dashboard messages
CREATE TABLE announcements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title VARCHAR(500) NOT NULL,
    content TEXT,
    type VARCHAR(20) NOT NULL DEFAULT 'message', -- message, update, maintenance
    is_active BOOLEAN NOT NULL DEFAULT true,
    published_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_announcements_active ON announcements(is_active);
CREATE INDEX idx_announcements_published ON announcements(published_at DESC);
CREATE INDEX idx_announcements_type ON announcements(type);
