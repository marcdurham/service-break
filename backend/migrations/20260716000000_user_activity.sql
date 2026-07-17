-- Account-activity audit log: logins, failed logins, and changes to a
-- user's own fields (username, password, admin flag, given/family name) —
-- whether made by the account itself or by an admin. Place edits and
-- ratings already have their own history (`place_edits`, `reviews`) and are
-- merged in at query time rather than duplicated here.
--
-- `user_id` is the account this activity is about and is cascade-deleted
-- with it (an activity page can only ever be reached for a live account).
-- `actor_id` is whoever performed the action (self or an admin) and is kept
-- as NULL if that account is later deleted, same as `place_edits.user_id`.
CREATE TABLE user_activity (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    activity_type TEXT NOT NULL,
    field TEXT NOT NULL DEFAULT '',
    old_value TEXT NOT NULL DEFAULT '',
    new_value TEXT NOT NULL DEFAULT '',
    actor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_user_activity_user ON user_activity (user_id, created_at DESC);
