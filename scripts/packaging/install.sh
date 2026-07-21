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
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
STAGE_ROOT="${TMPDIR:-/tmp}/moraine-install-stage-$$"
ROLLBACK_ROOT="${TMPDIR:-/tmp}/moraine-install-rollback-$$"

ACTIONS=()
ACTIONS+=("prefix=$PREFIX version=$VERSION")

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
  if [ -f "$BUNDLE_ROOT/systemd/moraine-service.service.in" ]; then
    mkdir -p "$STAGE_ROOT/systemd"
    sed "s|__MORAINE_SERVICE_BIN__|$LIBEXEC/moraine-service|g" \
      "$BUNDLE_ROOT/systemd/moraine-service.service.in" \
      > "$STAGE_ROOT/systemd/moraine-service.service"
  fi
}

backup_existing() {
  mkdir -p "$ROLLBACK_ROOT"
  for p in \
    "$BIN_DIR/moraine" \
    "$LIBEXEC" \
    "$LIB" \
    "$SHARE" \
    "$APP_SHARE/app.moraine.desktop" \
    "$ICON_DIR/app.moraine.png" \
    "$UNIT_DIR/moraine-service.service"
  do
    if [ -e "$p" ]; then
      rel=$(printf '%s' "$p" | sed 's|^/||')
      mkdir -p "$ROLLBACK_ROOT/$(dirname "$rel")"
      cp -a "$p" "$ROLLBACK_ROOT/$rel"
    fi
  done
}

rollback_install() {
  echo "error: install failed; rolling back previous suite files if present" >&2
  if [ -d "$ROLLBACK_ROOT" ]; then
    (
      cd "$ROLLBACK_ROOT"
      find . -type f -print0 2>/dev/null | while IFS= read -r -d '' f; do
        dest="/${f#./}"
        mkdir -p "$(dirname "$dest")"
        cp -a "$f" "$dest" 2>/dev/null || true
      done
    )
  fi
}

commit_stage() {
  mkdir -p "$BIN_DIR" "$LIBEXEC" "$LIB" "$SHARE" "$APP_SHARE" "$ICON_DIR" "$UNIT_DIR"
  install -m 755 "$STAGE_ROOT/bin/moraine" "$BIN_DIR/moraine"
  ACTIONS+=("installed $BIN_DIR/moraine")
  install -m 755 "$STAGE_ROOT/libexec/moraine/moraine-service" "$LIBEXEC/moraine-service"
  ACTIONS+=("installed $LIBEXEC/moraine-service")
  if [ -x "$STAGE_ROOT/lib/moraine/moraine-app" ]; then
    install -m 755 "$STAGE_ROOT/lib/moraine/moraine-app" "$LIB/moraine-app"
    ACTIONS+=("installed $LIB/moraine-app")
  fi
  mkdir -p "$SHARE"
  cp -a "$STAGE_ROOT/share/moraine/." "$SHARE/"
  ACTIONS+=("installed $SHARE")
  if [ -f "$STAGE_ROOT/share/applications/app.moraine.desktop" ]; then
    install -m 644 "$STAGE_ROOT/share/applications/app.moraine.desktop" \
      "$APP_SHARE/app.moraine.desktop"
    ACTIONS+=("desktop entry")
  fi
  if [ -f "$STAGE_ROOT/share/icons/hicolor/128x128/apps/app.moraine.png" ]; then
    install -m 644 "$STAGE_ROOT/share/icons/hicolor/128x128/apps/app.moraine.png" \
      "$ICON_DIR/app.moraine.png"
  fi
  if [ -f "$STAGE_ROOT/systemd/moraine-service.service" ]; then
    install -m 644 "$STAGE_ROOT/systemd/moraine-service.service" \
      "$UNIT_DIR/moraine-service.service"
    systemctl --user daemon-reload 2>/dev/null || true
    ACTIONS+=("systemd unit $UNIT_DIR/moraine-service.service")
  fi
}

if [ "$DRY_RUN" = 1 ]; then
  ACTIONS+=("would stage and install suite under $PREFIX")
  ACTIONS+=("would write unit with ExecStart=$LIBEXEC/moraine-service")
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
