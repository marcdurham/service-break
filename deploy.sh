#!/usr/bin/env bash
# Deploy the app stack to a remote Ubuntu/Docker host over SSH, using a
# docker context so the build runs on the remote daemon. See DEPLOY.md.
set -euo pipefail

REMOTE="${1:?Usage: ./deploy.sh user@host}"
CONTEXT_NAME="service-break-prod"

docker context inspect "$CONTEXT_NAME" >/dev/null 2>&1 \
  || docker context create "$CONTEXT_NAME" --docker "host=ssh://$REMOTE"

docker --context "$CONTEXT_NAME" compose \
  -f docker-compose.yml -f docker-compose.prod.yml \
  --profile app --env-file .env up --build -d

docker --context "$CONTEXT_NAME" compose \
  -f docker-compose.yml -f docker-compose.prod.yml --profile app ps
