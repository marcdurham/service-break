-- Soft-delete for invitations, mirroring places (20260712000000): removing
-- an already-expired invitation from the list hides it rather than
-- destroying the row, so the account-activity log entry it's paired with
-- still resolves to a real invitation if ever inspected directly.
ALTER TABLE invitations ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE invitations ADD COLUMN deleted_by UUID REFERENCES users(id) ON DELETE SET NULL;
