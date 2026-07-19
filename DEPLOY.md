# Deploying to an Ubuntu server

Ships the full app (`db` + `backend` + `frontend`) to a remote Ubuntu host that
already has Docker installed and is reachable over SSH. Deploys are triggered
manually by running `./deploy.sh`; there is no CI/CD pipeline.

How it works: `deploy.sh` points the local Docker CLI at the remote daemon via
a `docker context` over SSH, then runs `docker compose up --build`. The build
itself — compiling the Rust backend and the Yew/wasm frontend — happens **on
the server**, not on your machine; only the source tree is streamed over SSH.
There's no image registry involved.

**Resource note:** a release build of a Rust + wasm workspace needs a fair
amount of RAM. If the server is a small VPS (e.g. 1GB), the first build may be
slow or could OOM. Consider a swapfile or a larger instance if builds fail.

## One-time server setup

On the Ubuntu host:
1. Install Docker Engine + the Compose plugin.
2. Add your deploy user to the `docker` group so the remote context works
   without sudo: `sudo usermod -aG docker $USER`, then log out/in.
3. Make sure your SSH public key is authorized for that user
   (`~/.ssh/authorized_keys`).
4. If `ufw` is enabled, allow the frontend port: `sudo ufw allow 8020/tcp`.

## One-time local setup

1. Docker CLI installed locally (used only as a client — no local build load).
2. Copy `.env.example` to `.env` and set a real `POSTGRES_PASSWORD`. This file
   stays local; it's read by `docker compose` to interpolate
   `docker-compose.yml` before instructions are sent to the remote daemon.

## Deploying

```
./deploy.sh youruser@yourserver.example.com
```

Same command for the first deploy and every subsequent one. Only changed
layers rebuild; the `db-data` volume (Postgres data) persists across deploys.
The app becomes reachable at `http://<server-ip>:8020`.

Database migrations run automatically on backend startup — no manual step.

### Registration is invite-only

New accounts require a valid, unredeemed invite code (issued from the
"Invite your friends" button on the Account page of an existing account).
Codes expire 7 days after they are created. On a brand-new install with
zero users, nobody can issue one yet, so seed a one-off bootstrap
invitation directly:

```
docker --context service-break-prod compose \
  -f docker-compose.yml -f docker-compose.prod.yml --profile app \
  exec db psql -U service_break -d service_break \
  -c "INSERT INTO invitations (code) VALUES ('choose-a-one-time-code');"
```

Register the first account with that code, then invite everyone else from
the app. New accounts can invite immediately (limit: 25/day); after 24 hours
the limit rises to 100/day. To verify an old-account limit, backdate the
first account:

```
  ... exec db psql -U service_break -d service_break \
  -c "UPDATE users SET created_at = now() - interval '1 day' WHERE username = 'your-first-user';"
```

#### Revoking an invite code

If a user needs to revoke a sent invitation (e.g., the friend no longer
wants to join), they can do so from the Account page — pending invites
show a "Revoke" button. The revoked code becomes invalid immediately and
cannot be redeemed by anyone.

### Google sign-in (optional)

Lets people sign in — or register, still with an invite code — using their
Google account, alongside the existing username/password flow. It's off by
default; the buttons stay hidden until you configure a Google OAuth client.

#### 1. Create the OAuth client in Google Cloud Console

1. Go to <https://console.cloud.google.com/> and create a project (or pick an
   existing one) — top-left project switcher → **New Project**.
2. **APIs & Services → OAuth consent screen**:
   - User type **External** (unless you're on a Google Workspace domain and
     want it restricted to that org).
   - Fill in the app name, your support email, and developer contact email.
   - Scopes: the defaults (`openid`, `email`, `profile`) are enough — this
     app doesn't ask for anything beyond basic profile info.
   - While the app is in **Testing** mode, only test users you explicitly add
     (same screen) can sign in — fine for trying it out. Click **Publish App**
     when you want anyone with a Google account to be able to use it (Google
     may want basic verification for a public app, but this scope
     `openid`/`email`/`profile` combo is low-sensitivity and usually doesn't
     need a review).
3. **APIs & Services → Credentials → Create Credentials → OAuth client ID**:
   - Application type: **Web application**.
   - **Authorized redirect URIs** — add exactly (must match
     `GOOGLE_REDIRECT_URI` below, byte-for-byte, including the scheme):
     - Dev: `http://127.0.0.1:8020/api/auth/google/callback`
     - Prod: `http://<your-server-host-or-domain>:8020/api/auth/google/callback`
       (or `https://...` if you've put TLS in front of it)
   - Save — you'll get a **Client ID** and **Client secret**. Treat the
     secret like a password; it only ever lives server-side.

#### 2. Configure the backend

Set three environment variables (unset any one of them and Google sign-in
stays disabled — no partial/broken state):

- `GOOGLE_CLIENT_ID` — the Client ID from step 1.
- `GOOGLE_CLIENT_SECRET` — the Client secret from step 1.
- `GOOGLE_REDIRECT_URI` — the exact redirect URI you registered, e.g.
  `https://servicebreak.example.com/api/auth/google/callback`. The frontend
  origin is derived from this (strip the `/api/auth/google/callback` suffix),
  so it must be the same origin the app is actually served from.

For the Docker deploy (`./deploy.sh`), add these to your local `.env` (copied
from `.env.example`) — `docker compose` reads it to fill in
`docker-compose.yml` before sending build/run instructions to the server:

```
GOOGLE_CLIENT_ID=123456789-abc...apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=GOCSPX-...
GOOGLE_REDIRECT_URI=http://yourserver.example.com:8020/api/auth/google/callback
```

Then redeploy (`./deploy.sh youruser@yourserver.example.com`) so the backend
container picks them up. For local dev without Docker, export the same three
vars before `cargo run -p backend` (or add them to `.cargo/config.toml`'s
`[env]` table alongside `DATABASE_URL`), using
`GOOGLE_REDIRECT_URI=http://127.0.0.1:8020/api/auth/google/callback`.

#### How it behaves

- **Create an account** now shows "Continue with Google" once an invite code
  is entered — it redeems that code exactly like the normal registration
  form, just with a Google identity instead of a username/password. A
  username is generated automatically from the Google account's email.
- **Sign in** shows "Continue with Google" too, but it only ever signs in an
  account already linked to that Google identity — it can't be used to skip
  the invite code and create a new account.
- Accounts created this way have no password; they always sign in via
  Google. (Password accounts stay untouched — this doesn't change how
  existing users sign in.)

### Admin account — change the default password!

On first startup the backend creates an `admin` account with the default
password **`I brake for coffee`**. It's public knowledge (it's in this
file), so change it right after deploying:

```
COMPOSE="docker --context service-break-prod compose \
  -f docker-compose.yml -f docker-compose.prod.yml --profile app" \
  ./scripts/change-password.sh admin
```

The script prompts for the new password with hidden input, Argon2-hashes it
**locally** (using the same code the backend uses), and applies just the
`UPDATE` over the SSH docker context — the plaintext never travels to the
server. It works for any account, not just `admin`, and also revokes that
user's sessions. Without `COMPOSE=…` it targets the local dev database
(the docker Postgres on 127.0.0.1:5433).

### Backups (export → re-deploy → restore)

Sign in as `admin` and open **Account → Admin**:

1. **Export backup** downloads all data as a single JSON file (no password
   hashes, no session tokens).
2. Re-deploy / rebuild the app — even onto a fresh database.
3. Sign in as `admin` again (fresh installs recreate it with the default
   password) and **Import backup**: paste the file's contents and import.

Import replaces the database contents with the backup, keeping ids stable.
Accounts that don't exist yet are recreated with newly generated random
passwords, listed once on the page after the import — hand them out, or
reset them with `scripts/change-password.sh`. Accounts that already exist,
like the `admin` you're signed in as, keep their current password.

### Structured logging (OpenObserve)

The compose stack includes an [OpenObserve](https://openobserve.ai/)
container (`openobserve`, image `public.ecr.aws/zinclabs/openobserve`) that
collects the backend's logs — every REST request (method, route, status,
latency), logins/failed logins, registrations, Google sign-ins/links, place
create/update/delete, and tile-cache activity/errors. It's not
profile-gated, so it starts with a plain `docker compose up -d` alongside
`db`.

The backend ships logs to it automatically once deployed — no extra step —
by POSTing batched JSON to OpenObserve's `_json` ingestion endpoint (see
`backend/src/telemetry.rs`). Logs still also go to stdout as before
(`docker compose logs`), so nothing is lost if OpenObserve is unreachable.

- **Credentials**: set `OPENOBSERVE_USER` / `OPENOBSERVE_PASSWORD` in your
  `.env` (defaults to `admin@example.com` / `Ch4nge-Me-Please!` — change
  this before deploying, same as `POSTGRES_PASSWORD`; OpenObserve rejects
  weak passwords outright, so the default is only there to make first boot
  work, not to be secure). These double as the login for OpenObserve's web
  UI and the backend's ingestion credentials.
- **Viewing logs**: OpenObserve's UI listens on `127.0.0.1:5080` on the
  server (not exposed publicly). Reach it over an SSH tunnel:
  ```
  ssh -L 5080:127.0.0.1:5080 youruser@yourserver.example.com
  ```
  then open `http://127.0.0.1:5080` locally and sign in with
  `OPENOBSERVE_USER` / `OPENOBSERVE_PASSWORD`. Logs land in the `default`
  org's `backend` stream (override with `OPENOBSERVE_ORG` /
  `OPENOBSERVE_STREAM`).
- **Disabling it**: unset `OPENOBSERVE_URL` (or don't run the `openobserve`
  container) and the backend just logs to stdout — same as before this
  feature existed.
- **Local dev without Docker**: `cargo run -p backend` never ships to
  OpenObserve unless you export `OPENOBSERVE_URL` yourself (e.g.
  `http://127.0.0.1:5080` if you've started the container standalone).

## Rollback

```
git checkout <previous-sha>
./deploy.sh youruser@yourserver.example.com
```

Rebuilds and restarts from that commit. There's no separate image versioning
since nothing is pushed to a registry — rollback is just redeploying an older
commit.

## Logs / troubleshooting

```
docker --context service-break-prod compose \
  -f docker-compose.yml -f docker-compose.prod.yml --profile app logs -f
```
