-- Admin accounts: a flag on users. The "admin" account itself is created at
-- backend startup (see backend::admin::ensure_admin), because its default
-- password must be Argon2-hashed in code, not in SQL.
ALTER TABLE users ADD COLUMN is_admin BOOLEAN NOT NULL DEFAULT FALSE;
