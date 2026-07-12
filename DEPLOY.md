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
Invite tab by an existing account). On a brand-new install with zero users,
nobody can issue one yet, so seed a one-off bootstrap invitation directly:

```
docker --context service-break-prod compose \
  -f docker-compose.yml -f docker-compose.prod.yml --profile app \
  exec db psql -U service_break -d service_break \
  -c "INSERT INTO invitations (code) VALUES ('choose-a-one-time-code');"
```

Register the first account with that code, then invite everyone else from
the app.

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
