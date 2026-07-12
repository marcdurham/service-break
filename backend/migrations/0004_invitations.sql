-- Registration now requires a valid, unredeemed invitation code from an
-- existing user. inviter_id is nullable so an operator can hand-seed a
-- bootstrap invitation (no inviter) for a brand-new install with zero users
-- — see DEPLOY.md.
CREATE TABLE invitations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code TEXT NOT NULL,
    inviter_id UUID REFERENCES users(id) ON DELETE SET NULL,
    redeemed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    redeemed_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_invitations_code ON invitations (code);
CREATE INDEX idx_invitations_inviter ON invitations (inviter_id);
