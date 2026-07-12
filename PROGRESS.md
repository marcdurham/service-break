# Progress — Service Break Loop

## Current state (Iteration 1)

**Goal:** Implement four TODO items: user name fields, revised invite limits, invite revocation, and place sharing.

### M1 — User given/family name fields ✅ Complete
- [x] Migration `20260711140000_user_names.sql` adds `given_name` and `family_name` columns to users
- [x] Updated `UserRow` struct in `db.rs` to include new fields
- [x] Updated queries: `find_user`, `find_user_by_id`, `session_user` fetch the new columns
- [x] Added `update_user_names` function with partial-update logic (only writes fields that are `Some`)
- [x] Added `UpdateProfile` DTO and `validate_name` helper in `shared/src/lib.rs`
- [x] Added `PATCH /api/auth/profile` endpoint in `auth.rs`
- [x] Updated `GET /api/auth/me` to return the new fields
- [x] Frontend: Account view with Given/Family name inputs and Save button

### M2 — Revised invite limits ✅ Complete
- [x] Constants already set: `INVITES_PER_DAY_NEW = 25`, `INVITES_PER_DAY_OLD_AGE = 100`
- [x] Backend enforcement logic uses these constants (age gate determines limit, doesn't block)
- [x] Updated comments and DEPLOY.md documentation

### M3 — Revoke sent invite codes ✅ Complete
- [x] Migration `20260711150000_invitation_expired.sql` adds `is_expired` column to invitations
- [x] Added `revoke_invitation` function in `db.rs`
- [x] Added `DELETE /api/invites/{code}` endpoint in `auth.rs`
- [x] Frontend: Revoke button on pending invitations in Account view with toast notification

### M4 — Share place URI ✅ Complete
- [x] Added "Share" button in action row of Place Detail view
- [x] Copies current URL to clipboard via Web Clipboard API with toast confirmation

## Next steps
1. Complete M1 frontend (account view)
2. Start M2 (invite limits)
3. Then M3 and M4
