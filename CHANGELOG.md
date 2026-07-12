# Changelog

All notable changes to this project are logged here as they happen.
Newest entries at the top. Format: `YYYY-MM-DD HH:MM` (local time) —
short description.

## 2026-07-11

- 20:13 — Added admin accounts to the backend: an `is_admin` flag on users
  (migration `20260711120000`), an `admin` account auto-created at startup
  with the documented default password ("I brake for coffee" — change it!),
  and admin-only `GET /api/admin/export` / `POST /api/admin/import`
  endpoints. Export is one JSON document of all data minus sessions and
  password hashes; import restores it wholesale (ids preserved), recreating
  missing accounts with freshly generated random passwords that are
  returned once in the response. Sessions/`/me` now carry `is_admin`.

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
