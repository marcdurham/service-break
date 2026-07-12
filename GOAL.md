# GOAL — Four TODO items for Service Break

## Objective

Implement the four outstanding TODO items (not backlog) on the Service Break
Rust/Yew/Actix PWA: user name fields, revised invite limits, invite revocation,
and place sharing.

## Scope & Non-goals

**In scope:** Backend + frontend changes for exactly these four features.
No new pages beyond what each feature requires. No design overhaul — follow
the existing UI patterns (see `service-break.dc.html` and current components).

**Out of scope:** Backlog items, any refactor outside the touched code paths,
CI/CD changes, deployment scripts, or tests for unrelated endpoints.

## Milestones (small steps)

### M1 — User given/family name fields
- Migration: add `given_name TEXT NOT NULL DEFAULT ''` and `family_name TEXT NOT NULL DEFAULT ''` to `users`.
- Backend: `GET /api/auth/me` returns `{username, is_admin, given_name, family_name}`.
- Backend: `PATCH /api/auth/profile` body `{given_name?, family_name?}` — only fields present are updated; validates non-empty if sent (max 40 chars each). Auth required.
- Frontend: Account view shows Given/Family name inputs with a Save button; pre-filled from `/me`. On save, call `PATCH /api/auth/profile` and refresh the session.

### M2 — Revised invite limits
- Change constants in `shared/src/lib.rs`: `INVITE_WAIT_HOURS = 0`, `INVITES_PER_DAY_NEW = 25`, `INVITES_PER_DAY_OLD_AGE = 100`.
- Backend: `create_invitation` enforces the new rule — if account age < 24h, limit is 25/day; otherwise 100/day. Admins bypass both limits (keep existing behavior).
- Update error messages and docs (`DEPLOY.md`, README) to reflect new numbers.

### M3 — Revoke sent invite codes
- Backend: `DELETE /api/invites/{code}` — only the inviter can revoke their own pending invites; returns 204 on success, 404 if not found or not owned, 409 if already redeemed/expired.
- Frontend: Invite view lists sent invites with a "Revoke" button next to each pending one (disabled for expired/joined). Confirm before revoking. After revoke, refresh the list and mint a fresh code if needed.

### M4 — Share place URI
- Backend: no changes needed; share URL is client-side only (`/place/{id}`).
- Frontend: Add a "Share" button to the detail view action row (between "Map" and "Rate"). Uses `navigator.share()` with text like "{name} on Service Break — {url}" if available, falls back to clipboard copy. Shows toast "Link copied" or "Opening share…".

## Completion criteria

1. `cargo test --workspace` passes (all existing + new tests).
2. `cargo clippy --workspace --all-targets` clean.
3. `cargo clippy -p frontend --target wasm32-unknown-unknown` clean.
4. Each feature works end-to-end: backend API responds correctly, frontend renders and interacts properly.
5. New migrations added (never edit applied ones).
6. Tests added for new pure logic in `shared` and new/changed API behavior in `backend/tests/api.rs`.

## Quality standards

- **Tests:** Unit tests next to pure logic; integration tests (`#[sqlx::test]`) for every new or changed HTTP endpoint.
- **Docs:** Update README.md if any public API surface changes (it does — `/me` response shape).
- **Git:** Small, focused commits after each working change. Log every commit in CHANGELOG.md with date/time bullet.
- **Code style:** Follow existing patterns. Keep both native and wasm targets clippy-clean before committing.

## Assumptions

- Docker Postgres is running (required for backend tests).
- The design mockup `service-break.dc.html` is the reference for UI — follow its visual language.
- Nominatim must not be called in tests (already enforced via `http_client()` with unroutable URL).
- Existing invitation name field, expiry logic, and overview endpoint remain unchanged except for rate limits.
