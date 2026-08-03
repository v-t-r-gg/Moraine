#!/usr/bin/env bash
# Optional manual smoke: real Claude Code install → Moraine ProductCapture.
# Not a CI gate. Requires an authenticated `claude` on PATH.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLI="${MORAINE_CLI:-$ROOT/target/debug/moraine}"
if [[ ! -x "$CLI" ]]; then
  echo "build moraine first: cargo build -p moraine-cli" >&2
  exit 1
fi
if ! command -v claude >/dev/null 2>&1; then
  echo "claude not on PATH; skip real-provider smoke" >&2
  exit 2
fi
CLEAN=$(mktemp -d)
trap 'rm -rf "$CLEAN"' EXIT
PROJ="$CLEAN/proj"
mkdir -p "$PROJ"
"$CLI" project init "$PROJ"
echo "Claude version: $(claude --version 2>&1 | head -1)"
"$CLI" integrate claude-code --project "$PROJ" --json | head -c 2000
echo
"$CLI" integrate claude-code --project "$PROJ" --check --json | head -c 2000
echo
echo "Manual step: in another terminal, run a short non-destructive Claude Code session in $PROJ"
echo "Then re-run: $CLI doctor --project $PROJ --integration claude-code --json"
echo "Cleanup: $CLI integrate claude-code --project $PROJ --remove"
