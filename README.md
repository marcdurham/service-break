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
(`https://nominatim.openstreetmap.org`).

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
- 🔒 `POST /api/auth/logout`, `GET /api/auth/me`
- `GET  /api/geocode?q=`
- `GET  /api/health`

Accounts are username + password (Argon2-hashed) with 30-day session
tokens; changing anything requires signing in, browsing doesn't. The
anonymous per-device id in localStorage still scopes saved lists and
names reviews written before accounts existed.


## Deploying

See [DEPLOY.md](DEPLOY.md) for shipping the containerized stack to a remote
Ubuntu/Docker host over SSH.
