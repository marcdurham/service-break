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

### 5. ~~Add test for revoking nonexistent invitation returns 404~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New test verifies that attempting to revoke an invitation with a non-existent code returns 404.

### 6. ~~Add test for profile update validation~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New test verifies that attempting to update profile with empty name returns 400 Bad Request.

### 7. ~~Add test for profile update name length validation~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New test verifies that attempting to update profile with a name exceeding 40 characters returns 400 Bad Request.

### 8. ~~Add test for valid profile update~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New positive test verifies that updating profile with a valid name succeeds and the change is reflected in /api/auth/me.

### 9. ~~Add test for updating both given and family names~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New test verifies that updating profile with both given and family names in a single request succeeds and both values are persisted.

### 10. ~~Fix partial family name test to handle empty given_name~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** Updated test to accept either null or empty string for given_name field, since the backend may return empty strings instead of null values.

### 11. ~~Add test for rejecting both empty names in profile update~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New test verifies that attempting to update profile with both given and family names as empty strings returns 400 Bad Request.

### 12. ~~Add test for special characters in profile names~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New test verifies that names with apostrophes and hyphens (O'Brien, Smith-Jones) are accepted by the profile update endpoint.

### 13. ~~Add test for Unicode characters in profile names~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New test verifies that names with accented characters (José, García) are accepted by the profile update endpoint.

### 14. ~~Add test for whitespace trimming in profile names~~ ✅
**Files:** `backend/tests/api.rs`  
**Acceptance:** New test verifies that leading/trailing whitespace is properly handled when updating profile names.

### 15. ~~Fix whitespace-only name validation~~ ✅
**Files:** `shared/src/lib.rs`, `backend/tests/api.rs`  
**Acceptance:** Updated validate_name to trim input before checking emptiness, preventing whitespace-only strings from being accepted as valid names.

### 16. ~~Add clippy lints configuration for workspace~~ ✅
**Files:** `Cargo.toml` (workspace root)  
**Acceptance:** Workspace-level `[lints]` section configured with reasonable defaults; all crates compile without warnings under `cargo clippy --workspace --all-targets`. Already in place.

### 17. ~~Add test for admin invite limit bypass~~ ✅
**Files:** `backend/tests/api.rs`
**Acceptance:** New test `admin_invites_bypass_daily_limit` verifies that an admin user can send more than the daily invite limit (25+) without hitting the rate limit error.

### 18. ~~Add test for invite name length validation~~ ✅
**Files:** `backend/tests/api.rs`
**Acceptance:** New test `invite_name_too_long_rejected` verifies that names over 40 characters are rejected when creating an invitation, while names at or under the limit succeed.

### 19. ~~Add test for duplicate username registration rejection~~ ✅
**Files:** `backend/tests/api.rs`
**Acceptance:** New test `duplicate_username_rejected` verifies that attempting to register with a username that already exists returns 409 Conflict.

### 20. ~~Add test for partial given name profile update~~ ✅
**Files:** `backend/tests/api.rs`
**Acceptance:** New test `update_profile_partial_given_name` verifies that updating only the given name (without family name) succeeds and the change is reflected in /api/auth/me.

## Done

- (none yet)
