#!/usr/bin/env bash
# Remove Moraine suite product files; retain project ledgers and user spool by default.
# No Python required.
set -euo pipefail

PREFIX="${MORAINE_PREFIX:-$HOME/.local}"
DRY_RUN=0
JSON=0
PURGE=0

while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) PREFIX="$2"; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    --json) JSON=1; shift ;;
    --purge-user-state)
      PURGE=1
      shift
      ;;
    -h|--help)
      echo "Usage: ./uninstall.sh [--prefix DIR] [--dry-run] [--json] [--purge-user-state]"
      exit 0
      ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

UNIT="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/moraine-service.service"
ACTIONS=()

rm_path() {
  local p="$1"
  if [ ! -e "$p" ]; then return; fi
  if [ "$DRY_RUN" = 1 ]; then
    ACTIONS+=("would remove $p")
  else
    rm -rf "$p"
    ACTIONS+=("removed $p")
  fi
}

if [ "$DRY_RUN" = 0 ]; then
  systemctl --user stop moraine-service.service 2>/dev/null || true
  systemctl --user disable moraine-service.service 2>/dev/null || true
fi
rm_path "$UNIT"
if [ "$DRY_RUN" = 0 ]; then
  systemctl --user daemon-reload 2>/dev/null || true
fi

rm_path "$PREFIX/bin/moraine"
rm_path "$PREFIX/libexec/moraine"
rm_path "$PREFIX/lib/moraine"
rm_path "$PREFIX/share/moraine"
rm_path "$PREFIX/share/applications/app.moraine.desktop"
rm_path "$PREFIX/share/icons/hicolor/128x128/apps/app.moraine.png"

RETAINED=()
RETAINED+=("project-local .moraine/ directories (run ledgers)")
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/moraine-service"
if [ -d "$CACHE" ]; then
  if [ "$PURGE" = 1 ]; then
    rm_path "$CACHE"
  else
    RETAINED+=("$CACHE (spool/index; pass --purge-user-state to remove)")
  fi
fi

esc() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'; }

if [ "$JSON" = 1 ]; then
  echo "{"
  echo "  \"ok\": true,"
  echo "  \"dryRun\": $([ "$DRY_RUN" = 1 ] && echo true || echo false),"
  echo "  \"actions\": ["
  i=0
  for a in "${ACTIONS[@]}"; do
    i=$((i + 1))
    if [ "$i" -lt "${#ACTIONS[@]}" ]; then echo "    \"$(esc "$a")\","; else echo "    \"$(esc "$a")\""; fi
  done
  echo "  ],"
  echo "  \"retained\": ["
  i=0
  for r in "${RETAINED[@]}"; do
    i=$((i + 1))
    if [ "$i" -lt "${#RETAINED[@]}" ]; then echo "    \"$(esc "$r")\","; else echo "    \"$(esc "$r")\""; fi
  done
  echo "  ]"
  echo "}"
else
  echo "Moraine product files removed from $PREFIX"
  for a in "${ACTIONS[@]}"; do echo "  - $a"; done
  echo "Retained:"
  for r in "${RETAINED[@]}"; do echo "  - $r"; done
  echo "Codex project configs are not modified; use: moraine setup codex --project DIR --remove"
fi
