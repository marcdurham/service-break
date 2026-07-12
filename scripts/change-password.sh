#!/usr/bin/env bash
# Resets a Service Break user's password by writing a fresh Argon2 hash
# straight into the users table, and revokes that user's sessions.
#
# Usage:
#   ./scripts/change-password.sh <username> [password]
#
# If the password is omitted you are prompted for it (hidden input).
#
# By default this targets the local dev database (the docker Postgres on
# 127.0.0.1:5433, service "db" of docker-compose.yml). To run it against
# production, point COMPOSE at the remote docker context — the hash is
# computed locally by the `hash-password` backend binary, so only the
# UPDATE statement travels over SSH, never the plaintext password:
#
#   COMPOSE="docker --context service-break-prod compose \
#     -f docker-compose.yml -f docker-compose.prod.yml --profile app" \
#     ./scripts/change-password.sh admin
#
# Overridable env vars: COMPOSE (docker compose command), DB_SERVICE (db),
# POSTGRES_USER / POSTGRES_DB (service_break).
set -euo pipefail

cd "$(dirname "$0")/.."

if [ $# -lt 1 ] || [ $# -gt 2 ]; then
    echo "usage: $0 <username> [password]" >&2
    exit 2
fi
user="$1"

if [ $# -eq 2 ]; then
    password="$2"
else
    read -r -s -p "New password for ${user}: " password
    echo >&2
    read -r -s -p "Repeat to confirm: " confirm
    echo >&2
    if [ "$password" != "$confirm" ]; then
        echo "error: passwords do not match" >&2
        exit 1
    fi
fi

# Argon2-hash locally with the exact code the backend's login uses. The
# binary reads the password from stdin (keeps it out of ps/argv) and
# enforces the same minimum length as registration.
echo "Hashing (compiles the helper on first run)..." >&2
hash="$(printf '%s' "$password" | cargo run --quiet -p backend --bin hash-password)"

COMPOSE="${COMPOSE:-docker compose}"
DB_SERVICE="${DB_SERVICE:-db}"
DB_USER="${POSTGRES_USER:-service_break}"
DB_NAME="${POSTGRES_DB:-service_break}"

# The SQL is fed via stdin (not -c) because psql only interpolates
# variables there; :'user' / :'hash' are safely quoted by psql itself.
result="$($COMPOSE exec -T "$DB_SERVICE" \
    psql -U "$DB_USER" -d "$DB_NAME" \
    -v ON_ERROR_STOP=1 --no-align --tuples-only --quiet \
    -v "user=$user" -v "hash=$hash" <<'SQL'
WITH updated AS (
    UPDATE users SET password_hash = :'hash'
    WHERE lower(username) = lower(:'user')
    RETURNING id
),
revoked AS (
    DELETE FROM sessions WHERE user_id IN (SELECT id FROM updated)
)
SELECT count(*) FROM updated;
SQL
)"

case "$result" in
    1) echo "Password updated for ${user}; their sessions were signed out." ;;
    0) echo "error: no user named ${user}" >&2; exit 1 ;;
    *) echo "error: unexpected result from psql: ${result}" >&2; exit 1 ;;
esac
