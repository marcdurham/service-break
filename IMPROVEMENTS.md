# Improvements — Service Break

Concrete improvement items with file paths and acceptance criteria.

## Open Items

### 1. ~~Add test for invite revocation endpoint~~ ✅
**Files:** `backend/tests/api.rs`, `backend/src/db.rs`  
**Acceptance:** New test `revoked_invite_cannot_be_redeemed` verifies that a revoked invitation returns 400 when someone tries to register with it, and the invitation status changes to Expired.

### 2. ~~Update DEPLOY.md with invite revocation documentation~~ ✅
**Files:** `DEPLOY.md`  
**Acceptance:** Documentation includes section on revoking sent invitations.

### 3. Add clippy lints configuration for workspace
**Files:** `Cargo.toml` (workspace root)  
**Acceptance:** Workspace-level `[lints]` section configured with reasonable defaults; all crates compile without warnings under `cargo clippy --workspace --all-targets`.

## Done

- (none yet)
