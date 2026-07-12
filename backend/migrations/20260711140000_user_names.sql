-- Add given_name and family_name to users.
ALTER TABLE users ADD COLUMN given_name TEXT NOT NULL DEFAULT '';
ALTER TABLE users ADD COLUMN family_name TEXT NOT NULL DEFAULT '';
