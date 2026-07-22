#!/usr/bin/env bash
# C3 lifecycle smoke: reinstall, service restart/spool, uninstall retain ledger.
# Uses a temporary HOME under /tmp (not goal scratch). Requires prebuilt release bins
# or runs cargo build --release for cli+service.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CLEAN=$(mktemp -d /tmp/moraine-c3-life.XXXXXX)
echo "CLEAN=$CLEAN"
export HOME="$CLEAN"
export MORAINE_PREFIX="$CLEAN/.local"
export XDG_CONFIG_HOME="$CLEAN/.config"
export XDG_CACHE_HOME="$CLEAN/.cache"
export XDG_RUNTIME_DIR="$CLEAN/run"
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"
export PATH="$MORAINE_PREFIX/bin:/usr/bin:/bin"

if [ ! -x target/release/moraine ]; then
  cargo build --release -p moraine-cli -p moraine-service -q
fi

STAGE="$CLEAN/stage"
mkdir -p "$STAGE/bin" "$STAGE/systemd"
cp target/release/moraine target/release/moraine-service "$STAGE/bin/"
chmod 755 "$STAGE/bin/"*
cp crates/moraine-service/systemd/moraine-service.service.in "$STAGE/systemd/"
cp scripts/packaging/install.sh scripts/packaging/uninstall.sh "$STAGE/"
VERSION="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')"
VERSION="$VERSION" MORAINE_GIT_COMMIT="$(git rev-parse HEAD)" \
  python3 scripts/packaging/write_manifest.py "$STAGE"

echo "== install =="
"$STAGE/install.sh"
test -x "$MORAINE_PREFIX/bin/moraine"
moraine version --json >/dev/null

echo "== same-version reinstall =="
"$STAGE/install.sh" --json >/dev/null

echo "== project + service =="
PROJ="$CLEAN/proj"
mkdir -p "$PROJ"
moraine project init "$PROJ" --json
SOCK="$XDG_RUNTIME_DIR/moraine-service.sock"
SPOOL="$XDG_CACHE_HOME/moraine-service/spool"
HTTP=127.0.0.1:33201
"$MORAINE_PREFIX/libexec/moraine/moraine-service" \
  --http "$HTTP" --unix-socket "$SOCK" --spool-dir "$SPOOL" &
SPID=$!
sleep 1
curl -sf "http://$HTTP/status" >/dev/null

echo "== service stop + spool + restart =="
kill "$SPID"; wait "$SPID" 2>/dev/null || true
sleep 0.3
printf '%s\n' "{\"hook_event_name\":\"UserPromptSubmit\",\"session_id\":\"c3-life\",\"cwd\":\"$PROJ\",\"prompt\":\"lifecycle spool\"}" \
  | moraine hook-codex --socket "$SOCK" --spool-dir "$SPOOL"
find "$SPOOL" -maxdepth 1 -name 'event-*.json' | head
"$MORAINE_PREFIX/libexec/moraine/moraine-service" \
  --http "$HTTP" --unix-socket "$SOCK" --spool-dir "$SPOOL" &
SPID=$!
sleep 3
curl -sf "http://$HTTP/status"; echo
kill "$SPID" 2>/dev/null || true

echo "== uninstall retain ledger =="
echo keep > "$PROJ/.moraine/keep.txt"
"$STAGE/uninstall.sh"
test ! -e "$MORAINE_PREFIX/bin/moraine"
test -f "$PROJ/.moraine/keep.txt"
echo "C3_LIFECYCLE_SMOKE_OK"
