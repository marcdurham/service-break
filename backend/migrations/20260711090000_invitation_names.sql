-- Invitations carry a friendly name: the inviter can label who a code is
-- for, and the invited user can change it after registering. Expiry (7
-- days) and the per-day send limit are computed from created_at, so no
-- further schema is needed for them.
ALTER TABLE invitations ADD COLUMN name TEXT NOT NULL DEFAULT '';
