# Improvements — Service Break

Concrete improvement items with file paths and acceptance criteria.

## Open Items

### 1. Add test for invite revocation endpoint
**Files:** `backend/tests/api.rs`, `backend/src/db.rs`  
**Acceptance:** New test `revoked_invite_cannot_be_redeemed` verifies that a revoked invitation returns 400 when someone tries to register with it, and the invitation status changes to Expired.

### 2. Add integration test for share feature
**Files:** `frontend/tests/`, `shared/src/lib.rs`  
**Acceptance:** Test verifies that the Share button in detail view copies URL to clipboard and shows toast message.

### 3. Update DEPLOY.md with new invite limits documentation
**Files:** `DEPLOY.md`  
**Acceptance:** Documentation reflects current behavior: 25 invites/day for new accounts, 100/day after 24 hours, 7-day expiry.

## Done

- (none yet)
