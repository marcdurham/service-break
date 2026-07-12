# Improvements — Service Break

Concrete improvement items with file paths and acceptance criteria.

## Open Items

### 1. ~~Add test for invite revocation endpoint~~ ✅
**Files:** `backend/tests/api.rs`, `backend/src/db.rs`  
**Acceptance:** New test `revoked_invite_cannot_be_redeemed` verifies that a revoked invitation returns 400 when someone tries to register with it, and the invitation status changes to Expired.

### 2. ~~Update DEPLOY.md with invite revocation documentation~~ ✅
**Files:** `DEPLOY.md`  
**Acceptance:** Documentation includes section on revoking sent invitations.

### 3. ~~Add test for revoke endpoint authorization check~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New test `revoke_rejects_unauthorized_user` verifies that a non-inviter attempting to revoke an invitation receives a 404 response.

### 4. ~~Add test for revoking redeemed invitation fails~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New test `revoke_redeemed_invite_fails` verifies that attempting to revoke an already-redeemed invitation returns 404.

### 5. Add clippy lints configuration for workspace
**Files:** `Cargo.toml` (workspace root)  
**Acceptance:** Workspace-level `[lints]` section configured with reasonable defaults; all crates compile without warnings under `cargo clippy --workspace --all-targets`. Already in place.

## Done

- (none yet)
