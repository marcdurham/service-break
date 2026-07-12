-- Add is_expired flag to invitations for revocation support.
ALTER TABLE invitations ADD COLUMN IF NOT EXISTS is_expired BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX idx_invitations_is_expired ON invitations (is_expired) WHERE is_expired = true;
