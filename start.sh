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
  local tab_id root_pane
  tab_id=$(herdr tab list | jq -r --arg l "$label" \
    '.result.tabs[] | select(.label == $l) | .tab_id' | head -1)
  if [[ -z "$tab_id" ]]; then
    local created
    created=$(herdr tab create --cwd "$cwd" --label "$label" --no-focus)
    tab_id=$(jq -r '.result.tab.tab_id' <<<"$created")
    root_pane=$(jq -r '.result.root_pane.pane_id' <<<"$created")
    echo "Created new tab '$label': $tab_id" >&2
  else
    echo "Reusing existing tab '$label': $tab_id" >&2
    herdr tab focus "$tab_id" >/dev/null
    # tab list/get don't expose root_pane; look it up via pane list instead.
    root_pane=$(herdr pane list | jq -r --arg t "$tab_id" \
      '.result.panes[] | select(.tab_id == $t) | .pane_id' | head -1)
  fi
  echo "$root_pane"
}

# Check if docker compose is already running in the db tab.
# Looks for container names or "Attaching" text in recent output.
is_db_running() {
  local pane_id="$1"
  local output
  output=$(herdr pane read "$pane_id" --source recent --lines 20 2>/dev/null || true)
  if echo "$output" | grep -qE '(Attaching to|service-break-.*-1|\[.*\] Started)'; then
    return 0
  fi
  return 1
}

# Stop whatever is running in a pane (Ctrl+C), wait, then run new command.
restart_service() {
  local label="$1" pane_id="$2" cmd="$3"
  echo "Stopping existing $label service..."
  herdr pane send-keys "$pane_id" ctrl+c
  sleep 1
  herdr pane run "$pane_id" "$cmd"
  echo "Started $label on new port (pane $pane_id): $cmd"
}

for entry in "${SERVICES[@]}"; do
  IFS='|' read -r label cwd cmd <<<"$entry"
  pane_id=$(find_or_create_tab "$label" "$cwd")

  if [[ "$label" == "db" ]]; then
    # DB: only start if not already running (docker compose is long-lived)
    if is_db_running "$pane_id"; then
      echo "DB already running, skipping."
    else
      herdr pane run "$pane_id" "$cmd"
      echo "Started $label in tab (pane $pane_id): $cmd"
    fi
  else
    # Backend/frontend: always stop old process and start with new port
    restart_service "$label" "$pane_id" "$cmd"
  fi
done
