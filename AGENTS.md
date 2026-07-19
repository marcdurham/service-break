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

Run all three services with `./start.sh <N>` where N is a random 2-digit
number (0–99). This launches Postgres, backend (port **8000+N**), and frontend
(port **8800+N**) each in its own herdr tab labeled `db`, `backend`, and
`frontend`. `DATABASE_URL` is set in `.cargo/config.toml`.

After starting, check the herdr tabs to see service status — the `backend`
tab shows migration/seed output and the listening port; the `frontend` tab
shows the dev server URL. If something fails to start, read that tab's
output for errors.

Identity: username/password accounts (Argon2 hashes, 30-day bearer-token
sessions; see `backend/src/auth.rs`). All writes — adding places, posting
reviews, saving/unsaving — require login; reads stay public. An anonymous
per-device id in localStorage remains for scoping saved lists and naming
pre-account reviews. Place cleanliness is the average of its reviews'
`clean` scores (1–5).

## App places vs Overpass POIs/Places

The map and list views blend two kinds of location data — keep this
distinction in mind whenever touching places, filters, or the map:

- **App places** — rows in `places`: anything added directly through the
  app, *plus* any Overpass POI that has since been **promoted** because a
  user saved it, rated/reviewed it, or edited its details
  (`backend/src/handlers.rs::promote_overpass_poi`). App places can have
  reviews and a full edit history, and render in the **brown** pin/badge
  family (`PlaceType::color(PlaceSource::App)`). `places.source` records
  `"app"` vs `"overpass"` for provenance only — a promoted place is
  otherwise a fully normal app place, with no visible "from OSM" marker.
- **Overpass POIs** (aka "Overpass Places") — raw OpenStreetMap data pulled
  live from the Overpass API (fast food, cafés, stores, malls, parks) and
  cached per geohash tile in `overpass_tiles`/`overpass_pois`
  (`backend/src/overpass.rs`, `shared/src/tiles.rs`). They're read-only
  until promoted, capped at 250 per viewport (nearest to the query center),
  and render in the **blue/gray** pin/badge family
  (`PlaceType::color(PlaceSource::Overpass)`).

The first time a user saves, rates, or edits an Overpass POI, the backend
*promotes* it: a normal row is inserted into `places` and the original
`overpass_pois` cache row is linked to it via `app_place_id` rather than
deleted, so it's excluded from future Overpass layer results (never shown
twice) while the cache itself stays intact and its 7-day TTL keeps working.

The "Show unvisited places" toggle in the Filter modal (`FiltersSheet`,
`Filters.show_unvisited` in `frontend/src/app.rs`) controls whether the
Overpass POI layer shows at all — on the map, in the list, and in both
screens' search boxes. It's **on by default**.

## Commands

```sh
./start.sh 47                   # starts db + backend + frontend in herdr tabs
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
