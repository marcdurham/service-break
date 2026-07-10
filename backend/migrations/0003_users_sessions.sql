-- User accounts and bearer-token sessions. Writes (places, reviews, saved
-- changes) now require a logged-in user; reads stay public.
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Usernames are unique case-insensitively ("Sam" and "sam" collide).
CREATE UNIQUE INDEX idx_users_username ON users (lower(username));

CREATE TABLE sessions (
    token TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_sessions_user ON sessions (user_id);

-- Attribute new places and reviews to the account that wrote them. NULL on
-- rows from before accounts existed (and on seed data), where the anonymous
-- device id is still used for display.
ALTER TABLE places ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE reviews ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE SET NULL;
