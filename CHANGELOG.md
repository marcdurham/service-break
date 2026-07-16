# Changelog

All notable changes to this project are logged here as they happen.
Newest entries at the top. Format: `YYYY-MM-DD HH:MM` (local time) —
short description.

## 2026-07-16

- 21:50 — Removed speculative "Base .rate-btn (outside action-row)" row from STYLE-GUIDE button padding table; no .rate-btn exists outside .action-row in the codebase.
- 21:49 — Removed dead `padding: 15px 14px` from base `.rate-btn`; the action-row selector already sets it and every rate-btn lives inside an action-row.
- 21:49 — Updated styles-audit.md token table and contrast analysis to reflect current `--muted` (#6b5a48) and `--accent` (#934228) values; marked WCAG AA fixes as resolved.
- 21:45 — Standardized /place action-row button padding to `15px 14px` for all buttons (directions-btn and rate-btn) so Share/Delete buttons have consistent internal spacing with Map/Rate/Edit. Added `flex-wrap: wrap` to `.action-row` so buttons can float down to next line when no horizontal room. Documented both rules in STYLE-GUIDE.md.
- 21:30 — Fixed mobile overflow on /place action-row (Share/Delete buttons going off-screen): added `overflow-x: auto` with hidden scrollbar to `.action-row`, flex-basis constraints to button children, and text-overflow ellipsis for friend names/subtitles in account view. Added frontend style guide (`frontend/STYLE-GUIDE.md`) covering design tokens, spacing system, button sizes, typography scale, contrast requirements (WCAG AA), and component patterns; referenced it in `AGENTS.md`.
- 20:50 — Made Add tab button match other tabs (icon + label, no circle wrapper); removed dead `.tab-add-btn` CSS.
- 20:43 — Frontend style audit fixes: darken --muted (#8a7863→#6b5a48) and --accent (#c05f38→#934228) to pass WCAG AA 4.5:1 contrast; add icon size (.mi-sm/md/lg), avatar scale, and spacing utility classes; standardize .screen-title (28px), .screen-sub margin, .submit-btn / .alt-auth-btn heights; replace inline style="margin-*" across all component views with CSS classes.
- 03:05 — Fixed Users and Edit User pages layout: back button and title were squished inline; now matches /change-password with title on its own line and "Back to users" button below.

## 2026-07-15

- 11:11 — Added UX improvement analysis with three HTML files documenting issues found by comparing current implementation against design mockup. Includes visual comparisons, step-by-step implementation guide, and exact code changes reference.
- 17:00 — Removed `[[proxy]]` block from Trunk.toml; it conflicted with `--proxy-backend` by registering `/api/*` twice and panicking axum on startup.
- 20:17 — Fixed button text that didn't match its destination: Users page "Back to users" now says "Back to admin", and Account page's duplicate "Your name" section title is now "Your invitation name".
- 16:30 — Fixed EditUserView panic when navigating back from the edit page. The component was panicking because Yew hooks (use_state, Callback::from) were defined after an early return when not on the UserEdit route. Restructured so all hooks are initialized first, then check user_id_opt and return early if None.

## 2026-07-14

- 23:45 — Added admin user edit page with password reset and delete endpoints
- 23:07 — Simplified `auth::me` to use `.unwrap_or_default()` instead of match (clippy `manual_unwrap_or_default`).

## 2026-07-12

- 13:45 — Added a show/hide password toggle to the login form on `/account`, using the same `password_field` helper as the change-password section.

## 2026-07-11

- 00:50 — Added rename_invite_authorization test covering inviter/redeemer/unauthorized access to PUT /api/invites/{code}/name; fixed clippy format! suggestion.

- 00:45 — Added tests for partial given_name profile update, duplicate username rejection, and admin invite limit bypass. All 48 backend tests pass, workspace clippy-clean.

- 16:00 — M4 frontend complete (share place URI): Place Detail view now has a Share button in the action row that copies the current URL to clipboard via Web Clipboard API and shows a toast confirmation.

- 15:45 — M3 frontend complete (invite revocation UI): Account view now shows a Revoke button on each pending invitation; clicking it calls the new DELETE endpoint and refreshes the list with a toast.

- 15:30 — M3 backend complete (invite revocation): added `DELETE /api/invites/{code}` endpoint with migration for `is_expired` column on invitations table.

- 15:10 — M2 complete (revised invite limits): confirmed constants are already set to 25/day (new) and 100/day (old), backend enforcement uses them, updated comments and DEPLOY.md.

- 14:50 — Frontend for M1 (user given/family name fields): Account page now displays Given Name and Family Name inputs with a Save Profile button; loads names from `GET /api/auth/me` on sign-in; added `get_me()` and `update_profile()` API functions.

- 14:30 — Backend for M1 (user given/family name fields): migration `20260711140000_user_names.sql`, updated `UserRow` and all user queries, added `PATCH /api/auth/profile` endpoint with partial-update logic, extended `GET /api/auth/me` to return the new fields. Fixed invite-limit tests to use the new constants.

- 23:10 — Changed invitation limits per TODO #2: new accounts can send 25 invites/day immediately (no more 24-hour wait), and accounts older than 24 hours get 100/day. Admins bypass the limit entirely.

- 22:45 — Added show/hide password toggles (eye button) to the Password and Confirm password fields on `/register`, matching the existing toggle on the Account page's change-password section.

- 22:23 — Fixed admin accounts unable to create invite codes on fresh installs:
  `create_invitation` now skips the 24-hour age gate for admins, since they're
  auto-created at startup and need to invite immediately.

- 21:05 — Added /users page (admin-only) listing all accounts with a link from the admin menu.

- 20:40 — Renamed the admin-flag migration `20260711120000` →
  `20260711130000` to avoid a version collision with a same-numbered
  migration that landed on main.
- 20:33 — Added `scripts/change-password.sh <user> [password]` (TODO 117
  done): prompts with hidden input when the password is omitted, hashes it
  locally via the new `hash-password` backend binary (same Argon2 code the
  server uses), and applies the UPDATE to the running docker Postgres —
  also revoking the user's sessions. Works against production by setting
  `COMPOSE="docker --context service-break-prod compose …"`; documented in
  DEPLOY.md.
- 20:24 — Added the admin page to the frontend (TODO 120 done): signing in
  as an admin shows an Admin button on the Account page leading to
  `/admin`, where one button downloads the full backup as a JSON file and
  a paste-area imports one back — restored accounts' newly generated
  passwords are listed once after the import. Documented the admin
  account, its default password, and the export → re-deploy → restore
  flow in README.md and DEPLOY.md.
- 20:13 — Added admin accounts to the backend: an `is_admin` flag on users
  (migration `20260711130000`), an `admin` account auto-created at startup
  with the documented default password ("I brake for coffee" — change it!),
  and admin-only `GET /api/admin/export` / `POST /api/admin/import`
  endpoints. Export is one JSON document of all data minus sessions and
  password hashes; import restores it wholesale (ids preserved), recreating
  missing accounts with freshly generated random passwords that are
  returned once in the response. Sessions/`/me` now carry `is_admin`.
- 20:30 — Removed the completed invitation tasks (100-106, 116) from
  TODO.md and documented the new invite rules in DEPLOY.md (7-day code
  expiry, 24-hour wait and 5/day limit for senders, with a psql backdate
  snippet for bootstrapping the first account).
- 20:25 — Added a dedicated "Create an account" page at `/register`:
  username, password + confirm-password fields, and the invite code —
  pre-filled when the page is opened from an invitation link
  (`/register?code=XYZ`, which shared invites now point at). The Account
  page links to it when signed out; signed in, it gained an "Invite your
  friends" button (the Invite tab left the tab bar), a friends &
  invitations list with pending/expired/joined status, who invited you,
  and an editable name for your own invitation.
- 20:13 — Hardened invitations: codes now expire after 7 days, each user
  may send at most 5 per day, accounts younger than 24 hours can't invite
  yet, and codes are just the 8-character code (no `BREAK-` prefix).
  Invitations gained a name field — set by the inviter, editable by the
  invited user once registered (`PUT /api/invites/{code}/name`) — and
  `GET /api/invites` now returns a full overview (who invited you, your
  invitation's name, and each sent invite's pending/expired/joined status).
  The Invite page says "Invite your friends", labels the code "One time
  use code", takes an optional friend's name, and mints a fresh code after
  every copy or share.
- 20:12 — Reworked the top filter chips on the map and list screens:
  the six place-type chips (Shop, Store, Mall, Park, Public, Hall) are
  now tucked behind a single "Type" chip that expands them on tap, and
  the top level instead shows what places offer — Restroom, Coffee,
  Food, Seating — all selected by default. Deselecting chips narrows to
  places offering at least one of the remaining selections, via a new
  `amenities` parameter on `GET /api/places` (`&&` overlap on the
  existing `places.amenities` column; no schema change). Covered by a
  new `#[sqlx::test]` and shared-crate unit tests (TODO 113).
- 19:30 — Refreshed the onboarding (main entry) copy: tagline is now
  "find places for refreshment", headline "Good places to take breaks",
  intro "Places with coffee, food, bathrooms, places to sit at shops,
  stores, malls, parks & more", and the feature bullet reads
  "Cleanliness ratings" (TODO 112).
- 19:25 — Changed the app's main logo glyph from `wc` to `coffee`
  (onboarding badge, desktop nav-rail logo) and redrew the PWA icon
  (icon.svg + regenerated icon-192/512.png) as a steaming coffee cup
  in the same palette (TODO 107).
- 20:18 — Places are now editable by any logged-in user, with a full audit
  trail: `PUT /api/places/{id}` diffs the submitted fields against the
  stored row and writes one row per changed field to the new `place_edits`
  table (field, old/new value, editor, timestamp);
  `GET /api/places/{id}/edits` serves the history. The place detail page
  gained an Edit button (opens a prefilled full-screen editor; asks
  logged-out users to sign in) and a public "Change history" section. An
  unchanged address keeps the stored coordinates without re-geocoding;
  a no-op save writes no audit rows.
- 20:08 — Ratings are now aspect-specific: reviews keep the required
  bathroom-cleanliness score and can optionally score Coffee and Food
  (1-5, new nullable `coffee`/`food` columns on reviews). Place summaries
  expose per-aspect averages (`coffee_avg`/`food_avg`); the detail page's
  breakdown shows a bar per rated aspect, review cards show aspect chips,
  and the add-place form and review composer grew optional Coffee/Food
  pickers (tap the selected score again to clear it).

## 2026-07-10

- 20:35 — Containerized the app: multi-stage Dockerfiles for the backend
  (Rust build → slim Debian runtime) and frontend (Trunk wasm build →
  nginx serving the SPA and proxying `/api`), wired into
  docker-compose.yml behind an `app` profile so plain
  `docker compose up -d` still starts only Postgres;
  `docker compose --profile app up --build` runs the full stack on
  http://127.0.0.1:8080.
- 16:53 — Added an Account page (sign in / create account / sign out) and
  a sixth tab for it; the Add form, review composer and save button now
  ask logged-out users to sign in and route them to the Account page.
  Verified the whole flow end-to-end in the browser.
- 16:17 — Added user accounts to the backend: register/login/logout/me
  endpoints with Argon2 password hashing and 30-day bearer-token sessions;
  adding places, posting reviews, and saving/unsaving now require login,
  and new reviews are attributed to the account's username (older rows
  keep their anonymous scout names).
- 15:46 — Merged branch `geo-ux` into `main`; removed all completed items
  from TODO.md (the list is now empty).
- 15:37 — Merged branch `nav-invite` into `main`.
- 15:36 — Merged branch `place-model` into `main`.
- 15:35 — Distance filter now spans 1-100 mi on a logarithmic scale; the
  map's recenter button hugs the bottom-right corner (above the featured
  card when one is showing); the map picker shows the user's live location
  when they're sharing it.
- 15:34 — Added client-side routing (yew-router) with a URL per page and
  per-place deep links, plus a new Invite page reachable from the tab bar.
- 15:32 — Renamed "stop"/"stops" to "place"/"places" throughout the UI,
  API errors, and demo data; replaced business-category place types
  (Coffee, Grocery, Bookstore, Gas, Restroom) with generic venue types
  (Shop, Store, Mall, Public, Hall); added a multi-select amenities field
  (Restrooms, Coffee, Food, Groceries, Seating, Parking); and changed
  "Purchase required?"/"Code required?" from booleans to a three-state
  Yes/No/Don't know `Requirement`. Added migration `0002` to convert
  existing data and a new `amenities` column.
- 13:57 — Added this changelog and updated AGENTS.md to require dated
  entries alongside commits.

## Earlier

- Made the UI responsive with a mobile-first layout.
- Added map point-picking so a new place can be created without an address.
- Added "Show on map" to places.
