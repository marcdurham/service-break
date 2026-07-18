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

# Find or create a tab with the given label. Reuses an existing tab if one
# already exists (across any workspace); otherwise creates a new one.
find_or_create_tab() {
  local label="$1" cwd="$2"
  # Search all tabs for an exact label match
  local tab_id
  tab_id=$(herdr tab list | jq -r --arg l "$label" \
    '.result.tabs[] | select(.label == $l) | .tab_id' | head -1)
  if [[ -z "$tab_id" ]]; then
    tab_id=$(herdr tab create --cwd "$cwd" --label "$label" --no-focus \
      | jq -r '.result.tab.tab_id')
    echo "Created new tab '$label': $tab_id"
  else
    echo "Reusing existing tab '$label': $tab_id"
  fi
  # Ensure the tab's root pane is in the right cwd (in case it was moved)
  herdr tab focus "$tab_id" >/dev/null
  local root_pane
  root_pane=$(herdr tab list | jq -r --arg t "$tab_id" \
    '.result.tabs[] | select(.tab_id == $t) | .root_pane.pane_id' | head -1)
  echo "$root_pane"
}

for entry in "${SERVICES[@]}"; do
  IFS='|' read -r label cwd cmd <<<"$entry"
  pane_id=$(find_or_create_tab "$label" "$cwd")
  herdr pane run "$pane_id" "$cmd"
  echo "Running '$label': $cmd (pane $pane_id)"
done
