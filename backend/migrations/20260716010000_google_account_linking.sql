-- Let a signed-in user link a Google identity to their existing account
-- (or add a password to a Google-only account) instead of only being able
-- to choose one sign-in method at registration time. The OAuth round trip
-- for a "link" request carries the linking user's id through the same
-- short-lived table that already carries mode/invite_code.
ALTER TABLE oauth_states ADD COLUMN user_id UUID REFERENCES users(id) ON DELETE CASCADE;
