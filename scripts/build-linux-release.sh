#!/usr/bin/env bash
# Build a versioned, self-contained Moraine Linux x86_64 release suite (C2).
# Requires: Rust toolchain, Node/npm for desktop, optional WebKit deps for full app.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="${MORAINE_VERSION:-$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')}"
# Always embed the commit of the tree being packaged (dirty trees get -dirty suffix).
if [ -n "${MORAINE_GIT_COMMIT:-}" ]; then
  COMMIT="$MORAINE_GIT_COMMIT"
else
  COMMIT="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unknown)"
  if git -C "$ROOT" status --porcelain 2>/dev/null | grep -q .; then
    COMMIT="${COMMIT}-dirty"
  fi
fi
TARGET="${MORAINE_TARGET_TRIPLE:-x86_64-unknown-linux-gnu}"
OUT_DIR="${MORAINE_RELEASE_DIR:-$ROOT/dist}"
STAGE="$OUT_DIR/moraine-${VERSION}-linux-x86_64"
ARCHIVE="$OUT_DIR/moraine-${VERSION}-linux-x86_64.tar.gz"

export MORAINE_GIT_COMMIT="$COMMIT"
export MORAINE_TARGET_TRIPLE="$TARGET"
export MORAINE_BUILD_PROFILE=release
export RUSTFLAGS="${RUSTFLAGS:-}"

echo "Building Moraine $VERSION ($COMMIT) for $TARGET"

mkdir -p "$STAGE"/{bin,share/applications,share/icons/hicolor/128x128/apps,share/documentation,systemd,notices}

echo "==> cargo release binaries"
cargo build --release -p moraine-cli -p moraine-service

cp -f "$ROOT/target/release/moraine" "$STAGE/bin/moraine"
cp -f "$ROOT/target/release/moraine-service" "$STAGE/bin/moraine-service"
chmod 755 "$STAGE/bin/moraine" "$STAGE/bin/moraine-service"

# Primary product artifact REQUIRES the desktop (P0 no-terminal onboarding).
# Headless suites must use MORAINE_HEADLESS=1 and are named distinctly.
MORAINE_HEADLESS="${MORAINE_HEADLESS:-0}"
if [ "$MORAINE_HEADLESS" != "1" ]; then
  echo "==> desktop (required for primary product package)"
  if ! command -v npm >/dev/null; then
    echo "error: npm required to build moraine-app for the primary product package" >&2
    echo "  (set MORAINE_HEADLESS=1 for an explicitly named headless CLI+service package)" >&2
    exit 1
  fi
  npm ci --ignore-scripts 2>/dev/null || npm install --ignore-scripts
  npm run build
  if ! cargo build --release -p moraine-app; then
    echo "error: moraine-app release build failed — primary package will not ship headless" >&2
    exit 1
  fi
  if [ ! -f "$ROOT/target/release/moraine-app" ]; then
    echo "error: moraine-app binary missing after successful cargo build" >&2
    exit 1
  fi
  cp -f "$ROOT/target/release/moraine-app" "$STAGE/bin/moraine-app"
  chmod 755 "$STAGE/bin/moraine-app"
else
  echo "==> headless package (MORAINE_HEADLESS=1): skipping desktop"
  STAGE="$OUT_DIR/moraine-${VERSION}-linux-x86_64-headless"
  ARCHIVE="$OUT_DIR/moraine-${VERSION}-linux-x86_64-headless.tar.gz"
  mkdir -p "$STAGE"/{bin,share/applications,share/icons/hicolor/128x128/apps,share/documentation,systemd,notices}
  cp -f "$ROOT/target/release/moraine" "$STAGE/bin/moraine"
  cp -f "$ROOT/target/release/moraine-service" "$STAGE/bin/moraine-service"
  chmod 755 "$STAGE/bin/moraine" "$STAGE/bin/moraine-service"
fi

# Fail closed: primary package must contain desktop
if [ "$MORAINE_HEADLESS" != "1" ] && [ ! -f "$STAGE/bin/moraine-app" ]; then
  echo "error: primary product package missing bin/moraine-app" >&2
  exit 1
fi

# Desktop entry
cat > "$STAGE/share/applications/app.moraine.desktop" <<EOF
[Desktop Entry]
Name=Moraine
Comment=Local-first ledger for coding-agent runs
Exec=moraine-app
Icon=app.moraine
Terminal=false
Type=Application
Categories=Development;
EOF

if [ -f "$ROOT/src-tauri/icons/128x128.png" ]; then
  cp -f "$ROOT/src-tauri/icons/128x128.png" "$STAGE/share/icons/hicolor/128x128/apps/app.moraine.png"
fi

cp -f "$ROOT/crates/moraine-service/systemd/moraine-service.service.in" \
  "$STAGE/systemd/moraine-service.service.in"

cp -f "$ROOT/LICENSE" "$STAGE/LICENSE" 2>/dev/null || true
cp -f "$ROOT/SECURITY.md" "$STAGE/share/documentation/SECURITY.md" 2>/dev/null || true
cp -f "$ROOT/docs/REDACTION.md" "$STAGE/share/documentation/REDACTION.md" 2>/dev/null || true
cp -f "$ROOT/docs/integrations/CODEX.md" "$STAGE/share/documentation/CODEX.md" 2>/dev/null || true

# Manifest (single helper shared with CI)
VERSION="$VERSION" MORAINE_GIT_COMMIT="$COMMIT" MORAINE_TARGET_TRIPLE="$TARGET" \
  MORAINE_BUILD_PROFILE=release \
  python3 "$ROOT/scripts/packaging/write_manifest.py" "$STAGE"

# Install / uninstall scripts
cp -f "$ROOT/scripts/packaging/install.sh" "$STAGE/install.sh"
cp -f "$ROOT/scripts/packaging/uninstall.sh" "$STAGE/uninstall.sh"
chmod 755 "$STAGE/install.sh" "$STAGE/uninstall.sh"

# Checksums
(
  cd "$STAGE"
  find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS
)

mkdir -p "$OUT_DIR"
rm -f "$ARCHIVE"
tar -C "$OUT_DIR" -czf "$ARCHIVE" "$(basename "$STAGE")"
(
  cd "$OUT_DIR"
  sha256sum "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256"
)

echo "Bundle: $ARCHIVE"
ls -lh "$ARCHIVE"
cat "$ARCHIVE.sha256"
