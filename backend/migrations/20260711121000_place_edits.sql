-- Audit log for place edits. Any logged-in user can edit a place; every
-- changed field is recorded here — what changed (field + old/new value),
-- when, and by whom. user_id survives as NULL if the account is deleted.
CREATE TABLE place_edits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    place_id UUID NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    field TEXT NOT NULL,
    old_value TEXT NOT NULL,
    new_value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_place_edits_place ON place_edits (place_id, created_at DESC);
