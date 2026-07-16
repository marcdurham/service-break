# Service Break — agent instructions

## What this is

A mobile-first PWA for finding and rating clean bathrooms at coffee shops,
groceries, parks, gas stations, etc. Rust everywhere: Yew (wasm) frontend,
Actix Web backend, PostgreSQL. The visual design follows the prototype
mockup `service-break.dc.html` kept at the repo root — treat it as the
design reference when adding UI.

## Design

- **Style guide:** `frontend/STYLE-GUIDE.md` — design tokens, spacing system, button sizes, typography scale, contrast requirements (WCAG AA), and component patterns. Read it before adding or modifying UI.
- **Full audit:** `frontend/styles-audit.md` — page-by-page style inventory, contrast analysis, and consistency issues found during the last pass.
- The prototype mockup `service-break.dc.html` at the repo root is the source of truth for visual design; the style guide summarises it into actionable rules.

## Layout

| Path        | What it is                                                            |
|-------------|-----------------------------------------------------------------------|
| `shared/`   | DTOs + pure logic (haversine, lat/lng parsing) used by both sides     |
| `backend/`  | Actix Web API, sqlx/Postgres, Nominatim geocoding, `migrations/`, `seed.sql` |
| `frontend/` | Yew app served by Trunk; yew-router URL per page; Leaflet/OSM map via JS glue in `index.html`; styles in `assets/styles.css` |

Ports: frontend (Trunk) **8020**, backend API **8081**, Postgres (docker)
**127.0.0.1:5433**. `DATABASE_URL` is set in `.cargo/config.toml`.

Identity: username/password accounts (Argon2 hashes, 30-day bearer-token
sessions; see `backend/src/auth.rs`). All writes — adding places, posting
reviews, saving/unsaving — require login; reads stay public. An anonymous
per-device id in localStorage remains for scoping saved lists and naming
pre-account reviews. Place cleanliness is the average of its reviews'
`clean` scores (1–5).

## Commands

```sh
docker compose up -d            # Postgres (required for backend + its tests)
cargo run -p backend            # API on 127.0.0.1:8081; migrates + seeds on start
cd frontend && trunk serve      # app on http://127.0.0.1:8020, proxies /api
cargo test --workspace          # all tests
cargo clippy --workspace --all-targets
cargo clippy -p frontend --target wasm32-unknown-unknown
```

## Working conventions

- **Commit changes as you go.** Make small, focused commits after each
  working change lands (builds + tests pass) rather than batching
  everything into one commit at the end.
- **Log every commit in `CHANGELOG.md`.** Before or alongside each commit,
  add an entry under today's date (`## YYYY-MM-DD`, newest date at top) as
  a bullet `- HH:MM — short description`, using the current local
  date/time. Create a new date heading if today's isn't there yet.
- **TODO.md holds only outstanding work.** When a TODO item is completed,
  delete it from `TODO.md` (don't leave it checked off) and describe the
  completed work in `CHANGELOG.md`.
- **Add tests where needed.** New pure logic in `shared` gets unit tests
  next to it. New or changed API behavior gets a `#[sqlx::test]`
  integration test in `backend/tests/api.rs` (these create disposable
  databases on the docker Postgres, which must be running). Frontend
  logic that is testable belongs in `shared` where it can be unit-tested.
- Keep both native and wasm targets clippy-clean before committing.
- The backend runs migrations at startup; add schema changes as new files
  in `backend/migrations/`, never edit applied migrations.
- Nominatim (OpenStreetMap geocoding) requires a User-Agent header — it is
  set in `backend::http_client`; tests must not call the live service.
