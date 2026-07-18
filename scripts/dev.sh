#!/usr/bin/env bash
# Launch Moraine in development mode.
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ "${1:-}" == "cli" ]]; then
  shift
  exec cargo run -p moraine-cli -- "$@"
fi

if [[ "${1:-}" == "web" ]]; then
  exec npm run dev
fi

exec npm run tauri:dev
