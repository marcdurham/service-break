-- Google sign-in: link accounts to a Google subject id and email, and let
-- password_hash be NULL for accounts that only ever sign in via Google.
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;
ALTER TABLE users ADD COLUMN google_sub TEXT;
ALTER TABLE users ADD COLUMN email TEXT;

-- Plain unique indexes: Postgres never treats two NULLs as equal, so
-- existing password-only accounts (NULL google_sub/email) are unaffected.
CREATE UNIQUE INDEX idx_users_google_sub ON users (google_sub);
CREATE UNIQUE INDEX idx_users_email ON users (lower(email));

-- Short-lived, single-use state for the Google OAuth redirect round-trip:
-- CSRF protection, plus a place to carry `mode` (login vs. register) and the
-- invite code through the trip to Google and back, since Google only ever
-- echoes back the opaque `state` value we hand it.
CREATE TABLE oauth_states (
    state TEXT PRIMARY KEY,
    mode TEXT NOT NULL,
    invite_code TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);
