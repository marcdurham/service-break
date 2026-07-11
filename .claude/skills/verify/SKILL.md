---
name: verify
description: How to build, launch and drive Service Break to verify a change end-to-end (backend API + Yew frontend in a browser).
---

# Verifying Service Break changes

## Watch out: the user's own dev servers are often already running

Port 8081 (backend), 8020 (trunk) and the `service-break-db` container are
frequently occupied by a long-running dev session serving *other* code.
Don't kill them — run your build alongside on alternate ports:

```sh
docker compose up -d                      # errors if container exists; it's already up — fine
BIND_ADDR=127.0.0.1:8091 cargo run -p backend   # your code, own port
```

For the frontend, `trunk serve` reads `frontend/Trunk.toml` (port + /api
proxy). Temporarily edit it to `port = 8021` / proxy
`http://127.0.0.1:8091/api`, run `cd frontend && trunk serve`, and **revert
the file before committing**. First build takes a couple of minutes.

## Driving

- API surface: plain `curl` against `http://127.0.0.1:8091/api/...`.
  Auth'd writes need `Authorization: Bearer <token>` from
  `POST /api/auth/register` or `/login` (`{"username","password"}`).
- GUI surface: Claude-in-Chrome tools on `http://127.0.0.1:8021/`.
  localStorage is per-origin, so :8021 starts with a fresh device id /
  session — handy for testing logged-out flows.
- After a full page load the first click sometimes lands before the wasm
  app is interactive; if nothing happened, click once more.
- Toasts disappear after ~2.4 s — screenshot in the same batch as the
  click that triggers them.

## Cleanup

The backend on 8091 shares the dev database with the user's session.
Delete whatever rows you created, e.g.:

```sh
docker exec service-break-db psql -U service_break -d service_break \
  -c "DELETE FROM places WHERE name = '<your test place>'; DELETE FROM users WHERE username = '<your test user>';"
```

(`sessions`, `reviews`, `saved_places` cascade from users/places.)
Kill only your own servers (ports 8091/8021), revert `Trunk.toml`.
