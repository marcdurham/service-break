#!/usr/bin/env bash
# Starts the 3 services needed to debug this app (Postgres, backend API,
# frontend dev server), each in its own herdr tab.
# Usage: ./start.sh [0-99]
#   If a number between 0 and 99 is given, it's added as a suffix to the
#   default ports: backend uses 8000+N, frontend uses 8800+N.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

SUFFIX=0
if [[ $# -ge 1 ]]; then
  if [[ "$1" =~ ^[0-9]{1,2}$ ]] && (( 10#$1 < 100 )); then
    SUFFIX="$1"
  else
    echo "Error: argument must be a number between 0 and 99." >&2
    exit 1
  fi
fi

BACKEND_PORT=$((8000 + SUFFIX))
FRONTEND_PORT=$((8800 + SUFFIX))

# label, cwd, command
SERVICES=(
  "db|$ROOT|docker compose up"
  "backend|$ROOT|BIND_ADDR=127.0.0.1:${BACKEND_PORT} cargo run --bin backend"
  "frontend|$ROOT/frontend|trunk serve --port ${FRONTEND_PORT} --proxy-backend http://127.0.0.1:${BACKEND_PORT}/api"
)

for entry in "${SERVICES[@]}"; do
  IFS='|' read -r label cwd cmd <<<"$entry"
  pane_id=$(herdr tab create --cwd "$cwd" --label "$label" --no-focus \
    | jq -r '.result.root_pane.pane_id')
  herdr pane run "$pane_id" "$cmd"
  echo "Started $label in tab (pane $pane_id): $cmd"
done
