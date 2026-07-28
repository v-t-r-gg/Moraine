#!/usr/bin/env bash
# Install Moraine suite from an extracted release directory (C2).
# User-scoped by default. Idempotent. Does not require root, sudo, or Python.
set -euo pipefail

PREFIX="${MORAINE_PREFIX:-$HOME/.local}"
DRY_RUN=0
JSON=0

usage() {
  cat <<EOF
Usage: ./install.sh [--prefix DIR] [--dry-run] [--json]
Installs the Moraine suite into a user-scoped prefix (default: \$HOME/.local).
Does not require root. Does not delete project-local .moraine ledgers.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) PREFIX="$2"; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    --json) JSON=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage; exit 1 ;;
  esac
done

BUNDLE_ROOT="$(cd "$(dirname "$0")" && pwd)"
MANIFEST="$BUNDLE_ROOT/manifest.json"
if [ ! -f "$MANIFEST" ]; then
  echo "error: manifest.json missing in bundle root $BUNDLE_ROOT" >&2
  exit 1
fi
if [ ! -x "$BUNDLE_ROOT/bin/moraine" ] || [ ! -x "$BUNDLE_ROOT/bin/moraine-service" ]; then
  echo "error: bin/moraine and bin/moraine-service required and must be executable" >&2
  exit 1
fi

# Minimal JSON string field reader (no python). Expects "key": "value" on a line.
json_str() {
  local key="$1" file="$2"
  sed -n "s/.*\"${key}\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" "$file" | head -1
}

PRODUCT=$(json_str product "$MANIFEST")
VERSION=$(json_str version "$MANIFEST")
CLI_V=$(json_str cli "$MANIFEST")
SVC_V=$(json_str service "$MANIFEST")
DESK_V=$(json_str desktop "$MANIFEST")

if [ "$PRODUCT" != "Moraine" ]; then
  echo "error: manifest product must be Moraine (got: ${PRODUCT:-empty})" >&2
  exit 1
fi
if [ -z "$VERSION" ]; then
  echo "error: manifest version missing" >&2
  exit 1
fi
if [ "$CLI_V" != "$VERSION" ] || [ "$SVC_V" != "$VERSION" ]; then
  echo "error: components.cli/service must match version=$VERSION (cli=$CLI_V service=$SVC_V)" >&2
  exit 1
fi
if [ -n "$DESK_V" ] && [ "$DESK_V" != "$VERSION" ] && [ "$DESK_V" != "missing" ]; then
  echo "error: components.desktop=$DESK_V does not match version=$VERSION" >&2
  exit 1
fi

BIN_DIR="$PREFIX/bin"
LIBEXEC="$PREFIX/libexec/moraine"
LIB="$PREFIX/lib/moraine"
SHARE="$PREFIX/share/moraine"
APP_SHARE="$PREFIX/share/applications"
ICON_DIR="$PREFIX/share/icons/hicolor/128x128/apps"
UNIT="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/moraine-service.service"
STAGE_ROOT="${TMPDIR:-/tmp}/moraine-install-stage-$$"
ROLLBACK_ROOT="${TMPDIR:-/tmp}/moraine-install-rollback-$$"

ACTIONS=()
ACTIONS+=("prefix=$PREFIX version=$VERSION")
MANAGED_PATHS=(
  "$BIN_DIR/moraine"
  "$LIBEXEC"
  "$LIB"
  "$SHARE"
  "$APP_SHARE/app.moraine.desktop"
  "$ICON_DIR/app.moraine.png"
  "$UNIT"
)
ROLLBACK_DIRS=(
  "$ICON_DIR"
  "$PREFIX/share/icons/hicolor/128x128"
  "$PREFIX/share/icons/hicolor"
  "$PREFIX/share/icons"
  "$APP_SHARE"
  "$SHARE"
  "$PREFIX/share"
  "$LIB"
  "$PREFIX/lib"
  "$LIBEXEC"
  "$PREFIX/libexec"
  "$BIN_DIR"
  "$PREFIX"
)
PREEXISTED=()
DIR_PREEXISTED=()

cleanup_stage() {
  rm -rf "$STAGE_ROOT" "$ROLLBACK_ROOT" 2>/dev/null || true
}
trap cleanup_stage EXIT

stage_tree() {
  mkdir -p "$STAGE_ROOT"/{bin,libexec/moraine,lib/moraine,share/moraine,share/applications,share/icons/hicolor/128x128/apps}
  cp -f "$BUNDLE_ROOT/bin/moraine" "$STAGE_ROOT/bin/moraine"
  cp -f "$BUNDLE_ROOT/bin/moraine-service" "$STAGE_ROOT/libexec/moraine/moraine-service"
  chmod 755 "$STAGE_ROOT/bin/moraine" "$STAGE_ROOT/libexec/moraine/moraine-service"
  if [ -x "$BUNDLE_ROOT/bin/moraine-app" ]; then
    cp -f "$BUNDLE_ROOT/bin/moraine-app" "$STAGE_ROOT/lib/moraine/moraine-app"
    chmod 755 "$STAGE_ROOT/lib/moraine/moraine-app"
  fi
  # Copy manifest and inject prefix (no python dependency)
  {
    sed '$d' "$MANIFEST"
    printf '  ,"prefix": "%s"\n}\n' "$(printf '%s' "$PREFIX" | sed 's/\\/\\\\/g; s/"/\\"/g')"
  } > "$STAGE_ROOT/share/moraine/manifest.json"
  if [ -f "$BUNDLE_ROOT/LICENSE" ]; then
    cp -f "$BUNDLE_ROOT/LICENSE" "$STAGE_ROOT/share/moraine/LICENSE"
  fi
  if [ -d "$BUNDLE_ROOT/share/documentation" ]; then
    mkdir -p "$STAGE_ROOT/share/moraine/docs"
    cp -a "$BUNDLE_ROOT/share/documentation/." "$STAGE_ROOT/share/moraine/docs/"
  fi
  if [ -f "$BUNDLE_ROOT/share/applications/app.moraine.desktop" ]; then
    if [ -x "$STAGE_ROOT/lib/moraine/moraine-app" ]; then
      sed "s|^Exec=.*|Exec=$LIB/moraine-app|" \
        "$BUNDLE_ROOT/share/applications/app.moraine.desktop" \
        > "$STAGE_ROOT/share/applications/app.moraine.desktop"
    else
      cp -f "$BUNDLE_ROOT/share/applications/app.moraine.desktop" \
        "$STAGE_ROOT/share/applications/app.moraine.desktop"
    fi
  fi
  if [ -f "$BUNDLE_ROOT/share/icons/hicolor/128x128/apps/app.moraine.png" ]; then
    cp -f "$BUNDLE_ROOT/share/icons/hicolor/128x128/apps/app.moraine.png" \
      "$STAGE_ROOT/share/icons/hicolor/128x128/apps/app.moraine.png"
  fi
}

backup_existing() {
  mkdir -p "$ROLLBACK_ROOT" || return 1
  local i p
  for i in "${!MANAGED_PATHS[@]}"; do
    p="${MANAGED_PATHS[$i]}"
    if [ -e "$p" ]; then
      PREEXISTED[$i]=1
      cp -a "$p" "$ROLLBACK_ROOT/$i" || return 1
    else
      PREEXISTED[$i]=0
    fi
  done
  for i in "${!ROLLBACK_DIRS[@]}"; do
    if [ -d "${ROLLBACK_DIRS[$i]}" ]; then
      DIR_PREEXISTED[$i]=1
    else
      DIR_PREEXISTED[$i]=0
    fi
  done
}

rollback_install() {
  echo "error: install failed; rolling back previous suite files if present" >&2
  local i p
  for i in "${!MANAGED_PATHS[@]}"; do
    p="${MANAGED_PATHS[$i]}"
    rm -rf -- "$p" 2>/dev/null || true
    if [ "${PREEXISTED[$i]:-0}" = 1 ] && [ -e "$ROLLBACK_ROOT/$i" ]; then
      mkdir -p "$(dirname "$p")" 2>/dev/null || true
      cp -a "$ROLLBACK_ROOT/$i" "$p" 2>/dev/null || true
    fi
  done
  # Registration restoration is not visible to systemd until its cache reloads.
  systemctl --user daemon-reload 2>/dev/null || true
  for i in "${!ROLLBACK_DIRS[@]}"; do
    if [ "${DIR_PREEXISTED[$i]:-0}" = 0 ]; then
      rmdir -- "${ROLLBACK_DIRS[$i]}" 2>/dev/null || true
    fi
  done
}

commit_stage() {
  mkdir -p "$BIN_DIR" "$LIBEXEC" "$LIB" "$SHARE" "$APP_SHARE" "$ICON_DIR" || return 1
  install -m 755 "$STAGE_ROOT/bin/moraine" "$BIN_DIR/moraine" || return 1
  ACTIONS+=("installed $BIN_DIR/moraine")
  install -m 755 "$STAGE_ROOT/libexec/moraine/moraine-service" "$LIBEXEC/moraine-service" \
    || return 1
  ACTIONS+=("installed $LIBEXEC/moraine-service")
  if [ -x "$STAGE_ROOT/lib/moraine/moraine-app" ]; then
    install -m 755 "$STAGE_ROOT/lib/moraine/moraine-app" "$LIB/moraine-app" || return 1
    ACTIONS+=("installed $LIB/moraine-app")
  fi
  mkdir -p "$SHARE" || return 1
  cp -a "$STAGE_ROOT/share/moraine/." "$SHARE/" || return 1
  ACTIONS+=("installed $SHARE")
  if [ -f "$STAGE_ROOT/share/applications/app.moraine.desktop" ]; then
    install -m 644 "$STAGE_ROOT/share/applications/app.moraine.desktop" \
      "$APP_SHARE/app.moraine.desktop" || return 1
    ACTIONS+=("desktop entry")
  fi
  if [ -f "$STAGE_ROOT/share/icons/hicolor/128x128/apps/app.moraine.png" ]; then
    install -m 644 "$STAGE_ROOT/share/icons/hicolor/128x128/apps/app.moraine.png" \
      "$ICON_DIR/app.moraine.png" || return 1
  fi
  MORAINE_PREFIX="$PREFIX" "$BIN_DIR/moraine" service install --json >/dev/null || return 1
  ACTIONS+=("background runtime registration")
}

if [ "$DRY_RUN" = 1 ]; then
  ACTIONS+=("would stage and install suite under $PREFIX")
  ACTIONS+=("would register background runtime through $BIN_DIR/moraine")
else
  stage_tree
  backup_existing
  if ! commit_stage; then
    rollback_install
    exit 1
  fi
fi

if [ "$JSON" = 1 ]; then
  # Pure-bash JSON (escape paths minimally)
  esc() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'; }
  echo "{"
  echo "  \"ok\": true,"
  echo "  \"prefix\": \"$(esc "$PREFIX")\","
  echo "  \"version\": \"$(esc "$VERSION")\","
  echo "  \"dryRun\": $([ "$DRY_RUN" = 1 ] && echo true || echo false),"
  echo "  \"actions\": ["
  i=0
  for a in "${ACTIONS[@]}"; do
    i=$((i + 1))
    if [ "$i" -lt "${#ACTIONS[@]}" ]; then
      echo "    \"$(esc "$a")\","
    else
      echo "    \"$(esc "$a")\""
    fi
  done
  echo "  ],"
  echo "  \"pathHint\": \"ensure $BIN_DIR is on PATH before ~/.cargo/bin\","
  echo "  \"serviceStart\": \"not auto-started; run: moraine service start\""
  echo "}"
else
  echo "Moraine $VERSION installed to $PREFIX"
  for a in "${ACTIONS[@]}"; do echo "  - $a"; done
  echo
  echo "Next:"
  echo "  export PATH=\"$BIN_DIR:\$PATH\"   # if needed; prefer before ~/.cargo/bin"
  echo "  moraine version --verbose"
  echo "  moraine setup"
  echo "  moraine doctor"
  echo "  moraine setup codex --project /path/to/repo"
  echo
  echo "Service is not started automatically."
fi
