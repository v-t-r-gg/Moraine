#!/usr/bin/env bash
# Deterministic external-beta review fixture using public Moraine product surfaces.
# Does not hand-author run sidecars. Prints the project path on success.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PRESERVE=0
OUT_DIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --preserve) PRESERVE=1; shift ;;
    --out) OUT_DIR="$2"; shift 2 ;;
    -h|--help)
      echo "Usage: $0 [--preserve] [--out DIR]"
      echo "Creates a disposable review project with Codex + Claude + provisional runs."
      exit 0
      ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing $1" >&2; exit 1; }; }
need cargo
need python3

if [[ -z "${MORAINE_BIN:-}" ]]; then
  cargo build -p moraine-cli -p moraine-service -q --manifest-path "$ROOT/Cargo.toml"
  MORAINE_BIN="$ROOT/target/debug/moraine"
  SERVICE_BIN="$ROOT/target/debug/moraine-service"
else
  SERVICE_BIN="${MORAINE_SERVICE_BIN:-$(dirname "$MORAINE_BIN")/moraine-service}"
fi
test -x "$MORAINE_BIN"
test -x "$SERVICE_BIN" || { echo "missing moraine-service next to moraine" >&2; exit 1; }

if [[ -n "$OUT_DIR" ]]; then
  WORK="$OUT_DIR"
  mkdir -p "$WORK"
else
  WORK="$(mktemp -d "${TMPDIR:-/tmp}/moraine-review-fx.XXXXXX")"
fi
PROJECT="$WORK/review-project"
mkdir -p "$PROJECT" "$WORK/bin" "$WORK/spool" "$WORK/fx"
SPOOL="$WORK/spool"
SOCK="$WORK/cap.sock"
HTTP_PORT="$(python3 - <<'PY'
import socket
s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()
PY
)"

cleanup() {
  if [[ -n "${SVC_PID:-}" ]]; then kill "$SVC_PID" 2>/dev/null || true; wait "$SVC_PID" 2>/dev/null || true; fi
  if [[ "$PRESERVE" -eq 0 && -z "$OUT_DIR" ]]; then rm -rf "$WORK"; fi
}
trap cleanup EXIT

# Stage suite-like layout for hook adapters
cp "$MORAINE_BIN" "$WORK/bin/moraine"
cp "$SERVICE_BIN" "$WORK/bin/moraine-service"
chmod +x "$WORK/bin/moraine" "$WORK/bin/moraine-service"
cat > "$WORK/fx/codex" <<'SH'
#!/bin/sh
echo "codex fixture 0.0.0"
SH
cat > "$WORK/fx/claude" <<'SH'
#!/bin/sh
echo "claude fixture 0.0.0"
SH
chmod +x "$WORK/fx/codex" "$WORK/fx/claude"
export PATH="$WORK/fx:$WORK/bin:$PATH"

"$WORK/bin/moraine-service" --http "127.0.0.1:${HTTP_PORT}" --unix-socket "$SOCK" --spool-dir "$SPOOL" \
  >/dev/null 2>&1 &
SVC_PID=$!
for _ in $(seq 1 50); do
  if curl -sf "http://127.0.0.1:${HTTP_PORT}/status" | grep -q online; then break; fi
  sleep 0.1
done

"$MORAINE_BIN" project init "$PROJECT" >/dev/null

hook() {
  local sub="$1"; shift
  printf '%s' "$1" | "$MORAINE_BIN" "$sub" --socket "$SOCK" --spool-dir "$SPOOL" >/dev/null
}

# --- Codex-bound run (mechanical + tool) ---
# Configure adapters via provision-style integrate when available; else skip apply.
if "$MORAINE_BIN" integrate codex --project "$PROJECT" >/dev/null 2>&1; then :; fi
if "$MORAINE_BIN" integrate claude-code --project "$PROJECT" >/dev/null 2>&1; then :; fi

VID="review-codex-fixture"
SESSION_X="review-codex-session"
hook hook-codex "$(python3 - <<PY
import json
print(json.dumps({
  "hook_event_name": "SessionStart",
  "session_id": "$SESSION_X",
  "cwd": "$PROJECT",
  "event_id": "$VID-s",
}))
PY
)"
hook hook-codex "$(python3 - <<PY
import json
print(json.dumps({
  "hook_event_name": "UserPromptSubmit",
  "session_id": "$SESSION_X",
  "cwd": "$PROJECT",
  "event_id": "$VID-p",
  "prompt": "Moraine self-test verification_id=$VID review workspace Codex run",
}))
PY
)"
hook hook-codex "$(python3 - <<PY
import json
print(json.dumps({
  "hook_event_name": "PreToolUse",
  "session_id": "$SESSION_X",
  "cwd": "$PROJECT",
  "event_id": "$VID-t",
  "tool_name": "Bash",
  "tool_use_id": "call-1",
  "tool_input": {"command": "echo fixture"},
}))
PY
)"
sleep 0.6

# Confirm + checkpoint via CLI
RUN_JSON="$("$MORAINE_BIN" run start --project "$PROJECT" \
  --objective "Review workspace Codex fixture: ship discovery filters" \
  --idempotency-key "fx-codex-start" \
  --session-id "$SESSION_X" --json)"
RUN_ID="$(python3 - <<PY
import json,sys
j=json.loads('''$RUN_JSON''')
print(j.get("run",{}).get("id") or j.get("runId") or "")
PY
)"
HASH="$(python3 - <<PY
import json
j=json.loads('''$RUN_JSON''')
print(j.get("run",{}).get("contentHash") or "")
PY
)"
if [[ -z "$HASH" && -n "$RUN_ID" ]]; then
  SHOW="$("$MORAINE_BIN" run show --run-id "$RUN_ID" --project "$PROJECT" --json)"
  HASH="$(python3 - <<PY
import json
j=json.loads('''$SHOW''')
print(j.get("run",{}).get("contentHash") or j.get("contentHash") or "")
PY
)"
fi
if [[ -n "$RUN_ID" && -n "$HASH" ]]; then
  CP_FILE="$WORK/cp-codex.json"
  cat > "$CP_FILE" <<'JSON'
{
  "summary": "Codex fixture checkpoint with risk and question",
  "risks": ["Fixture risk: filter edge case"],
  "openQuestions": ["Should empty objectives show a placeholder?"],
  "evidence": [
    {"kind": "note", "label": "agent claim for fixture", "provenance": "agent_reported"}
  ]
}
JSON
  "$MORAINE_BIN" run checkpoint --run-id "$RUN_ID" --project "$PROJECT" \
    --expected-hash "$HASH" --idempotency-key "fx-codex-cp" --input "$CP_FILE" --json >/dev/null || true
fi

# --- Claude Code run ---
VID2="review-claude-fixture"
SESSION_C="review-claude-session"
hook hook-claude-code "$(python3 - <<PY
import json
print(json.dumps({
  "hook_event_name": "SessionStart",
  "session_id": "$SESSION_C",
  "cwd": "$PROJECT",
  "event_id": "$VID2-s",
}))
PY
)"
hook hook-claude-code "$(python3 - <<PY
import json
print(json.dumps({
  "hook_event_name": "UserPromptSubmit",
  "session_id": "$SESSION_C",
  "cwd": "$PROJECT",
  "event_id": "$VID2-p",
  "prompt": "Moraine self-test verification_id=$VID2 review workspace Claude run",
}))
PY
)"
sleep 0.5
RUN2_JSON="$("$MORAINE_BIN" run start --project "$PROJECT" \
  --objective "Review workspace Claude fixture: document fidelity gaps" \
  --idempotency-key "fx-claude-start" \
  --session-id "$SESSION_C" --json || true)"
RUN2_ID="$(python3 - <<PY
import json
j=json.loads('''${RUN2_JSON:-{}}''' or '{}')
print(j.get("run",{}).get("id") or "")
PY
)"
HASH2="$(python3 - <<PY
import json
j=json.loads('''${RUN2_JSON:-{}}''' or '{}')
print(j.get("run",{}).get("contentHash") or "")
PY
)"
if [[ -n "$RUN2_ID" && -n "$HASH2" ]]; then
  CP2="$WORK/cp-claude.json"
  cat > "$CP2" <<'JSON'
{
  "summary": "Claude fixture checkpoint",
  "evidence": [
    {"kind": "note", "label": "claude agent-reported note", "provenance": "agent_reported"}
  ]
}
JSON
  "$MORAINE_BIN" run checkpoint --run-id "$RUN2_ID" --project "$PROJECT" \
    --expected-hash "$HASH2" --idempotency-key "fx-claude-cp" --input "$CP2" --json >/dev/null || true
fi

# --- Provisional / mechanical-only run ---
VID3="review-provisional-fixture"
hook hook-codex "$(python3 - <<PY
import json
print(json.dumps({
  "hook_event_name": "SessionStart",
  "session_id": "review-prov-session",
  "cwd": "$PROJECT",
  "event_id": "$VID3-s",
}))
PY
)"
hook hook-codex "$(python3 - <<PY
import json
print(json.dumps({
  "hook_event_name": "UserPromptSubmit",
  "session_id": "review-prov-session",
  "cwd": "$PROJECT",
  "event_id": "$VID3-p",
  "prompt": "Moraine self-test verification_id=$VID3 provisional only",
}))
PY
)"
sleep 0.4

# Leave provisional (no run start) for fidelity gaps.
# Print project path for consumers.
echo "$PROJECT"
# Preserve for caller when --preserve or --out
if [[ "$PRESERVE" -eq 1 || -n "$OUT_DIR" ]]; then
  trap - EXIT
  if [[ -n "${SVC_PID:-}" ]]; then kill "$SVC_PID" 2>/dev/null || true; fi
fi
