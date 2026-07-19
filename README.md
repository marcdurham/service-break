# Service Break

Find a clean place: real bathroom ratings at shops, stores, malls, parks
& more — sorted by what's closest to you.

A mobile-first PWA built in Rust: [Yew](https://yew.rs) frontend with
[Leaflet](https://leafletjs.com) + OpenStreetMap tiles, an
[Actix Web](https://actix.rs) API, and PostgreSQL. The design follows
`service-break.dc.html` (prototype mockup, kept in the repo as reference).

## Stack

| Crate      | What it is                                                        |
|------------|-------------------------------------------------------------------|
| `shared`   | DTOs + pure logic (haversine, lat/lng parsing) used by both sides |
| `backend`  | Actix Web API, sqlx/Postgres, Nominatim geocoding                 |
| `frontend` | Yew (wasm) PWA served by Trunk, Leaflet map via a small JS glue   |

## Running it

Prerequisites: [Docker](https://docs.docker.com/get-docker/) (for Postgres)
and a stable [Rust toolchain](https://rustup.rs), plus the wasm target and
[Trunk](https://trunkrs.dev) for the frontend:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
```

Then, in three terminals from the repo root:

```sh
# 1. Database (Postgres 16 on 127.0.0.1:5433)
docker compose up -d

# 2. Backend API on 127.0.0.1:8081 (runs migrations + demo seed on startup)
cargo run -p backend

# 3. Frontend on http://127.0.0.1:8020 (proxies /api to the backend)
cd frontend && trunk serve
```

Then open <http://127.0.0.1:8020>. Demo seed data is placed around downtown
Seattle; deny the location prompt (or be elsewhere) and the app falls back to
that area.

Config via env (defaults in parentheses): `DATABASE_URL` (set in
`.cargo/config.toml`), `BIND_ADDR` (`127.0.0.1:8081`), `NOMINATIM_URL`
(`https://nominatim.openstreetmap.org`). "Sign in with Google" is optional
and off unless `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` and
`GOOGLE_REDIRECT_URI` are all set — see DEPLOY.md for how to get them.
Shipping logs to [OpenObserve](https://openobserve.ai/) is likewise optional
and off unless `OPENOBSERVE_URL` is set — see DEPLOY.md.

## Tests

```sh
cargo test --workspace
```

Backend integration tests use `#[sqlx::test]`, which creates a disposable
database per test on the docker Postgres — it must be running.

## API

Endpoints marked 🔒 require an `Authorization: Bearer <token>` header from
register/login.

- `GET  /api/places?q&lat&lng&radius_mi&types&clean_min&no_purchase&has_parking&sort`
  — `q` searches place names and addresses (case-insensitive)
- 🔒 `POST /api/places` — body includes address **or** `lat`/`lng`; free-text
  addresses are geocoded via Nominatim (OpenStreetMap)
- `GET  /api/places/{id}?lat&lng`
- 🔒 `POST /api/places/{id}/reviews`
- `GET /api/devices/{device_id}/saved` · 🔒 `PUT/DELETE /api/devices/{device_id}/saved/{place_id}`
- `POST /api/auth/register`, `POST /api/auth/login` — body
  `{ "username", "password" }`, return `{ "token", "username" }`
- 🔒 `POST /api/auth/logout`, `GET /api/auth/me` — returns `{ "token", "username", "is_admin", "given_name", "family_name" }`
- `GET /api/auth/google/enabled`, `GET /api/auth/google/start?mode&invite_code`,
  `GET /api/auth/google/callback` — "Sign in with Google" (see DEPLOY.md);
  `start`/`callback` are browser redirects, not JSON endpoints
- 🔒 `GET /api/admin/export`, `POST /api/admin/import` — admin only (403
  otherwise); see "Admin account" below
- `GET  /api/geocode?q=`
- `GET  /api/health`

Accounts are username + password (Argon2-hashed) or linked to a Google
identity, with 30-day session tokens either way; changing anything requires
signing in, browsing doesn't. The
anonymous per-device id in localStorage still scopes saved lists and
names reviews written before accounts existed.

## Admin account

On startup the backend creates an `admin` account (if none exists) with the
default password **`I brake for coffee`** — change it immediately with
`./scripts/change-password.sh admin` (see below). Signing in as an admin
adds an **Admin** button to the Account page, which opens the admin menu:

- **Export backup** — downloads every table (places, reviews, saved lists,
  invitations, accounts) as one JSON file. Password hashes and session
  tokens are never exported.
- **Import backup** — paste a backup file's contents to restore it. Content
  is replaced wholesale with ids preserved (deep links keep working).
  Accounts that don't exist yet are recreated with freshly generated random
  passwords, shown once after the import; accounts that already exist
  (matched by username — including the importing admin) keep their current
  password.

Together these make "export → re-deploy → import" a quick way to move data.

## Resetting a password

`./scripts/change-password.sh <username> [password]` writes a fresh Argon2
hash for `<username>` straight into the running database (and revokes their
sessions). Omit the password to be prompted with hidden input. It targets
the local docker Postgres by default; see DEPLOY.md for running it against
production.


## Deploying

See [DEPLOY.md](DEPLOY.md) for shipping the containerized stack to a remote
Ubuntu/Docker host over SSH.
