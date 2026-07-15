# Progress — Service Break Loop

## Current state — ALL COMPLETE ✅

**Goal:** Implement four TODO items: user name fields, revised invite limits, invite revocation, and place sharing.

### M1 — User given/family name fields ✅
- Migration `20260711140000_user_names.sql`, `PATCH /api/auth/profile`, updated `/me` response, frontend account view inputs + Save.

### M2 — Revised invite limits ✅
- Constants `INVITES_PER_DAY_NEW = 25`, `INVITES_PER_DAY_OLD_AGE = 100`; backend enforces age gate; docs updated.

### M3 — Revoke sent invite codes ✅
- Migration `20260711150000_invitation_expired.sql`, `DELETE /api/invites/{code}`, frontend revoke button with confirmation + toast.

### M4 — Share place URI ✅
- Frontend "Share" button on Place Detail using `navigator.share()` with clipboard fallback and toast.

## Quality gate
- `cargo test --workspace`: 32 passed, 0 failed.
- `cargo clippy --workspace --all-targets`: clean.
- `cargo clippy -p frontend --target wasm32-unknown-unknown`: clean.
