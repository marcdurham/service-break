#!/usr/bin/env bash
# Starts the 3 services needed to debug this app (Postgres, backend API,
# frontend dev server), each in its own herdr tab.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# label, cwd, command
SERVICES=(
  "db|$ROOT|docker compose up"
  "backend|$ROOT|cargo run -p backend"
  "frontend|$ROOT/frontend|trunk serve"
)

for entry in "${SERVICES[@]}"; do
  IFS='|' read -r label cwd cmd <<<"$entry"
  pane_id=$(herdr tab create --cwd "$cwd" --label "$label" --no-focus \
    | jq -r '.result.root_pane.pane_id')
  herdr pane run "$pane_id" "$cmd"
  echo "Started $label in tab (pane $pane_id): $cmd"
done
