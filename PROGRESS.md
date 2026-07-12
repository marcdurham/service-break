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

### M2 — Revised invite limits
- [ ] Change constants: `INVITE_WAIT_HOURS = 0`, `INVITES_PER_DAY_NEW = 25`, `INVITES_PER_DAY_OLD_AGE = 100`
- [ ] Update backend enforcement logic (already partially done)
- [ ] Update error messages and docs

### M3 — Revoke sent invite codes
- [ ] Backend: `DELETE /api/invites/{code}` endpoint
- [ ] Frontend: Invite view with Revoke button for pending invites

### M4 — Share place URI
- [ ] Frontend: Add "Share" button to detail view action row
- [ ] Use `navigator.share()` or clipboard fallback

## Next steps
1. Complete M1 frontend (account view)
2. Start M2 (invite limits)
3. Then M3 and M4
