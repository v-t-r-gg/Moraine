#!/usr/bin/env bash
# Install Arch Linux dependencies for Moraine (Tauri 2 + Rust + Node).
set -euo pipefail

echo "==> Moraine Arch setup"

if ! command -v pacman >/dev/null 2>&1; then
  echo "This script is intended for Arch Linux (pacman not found)."
  exit 1
fi

echo "==> System packages (sudo)"
sudo pacman -S --needed --noconfirm \
  base-devel \
  curl \
  wget \
  file \
  openssl \
  appmenu-gtk-module \
  libappindicator-gtk3 \
  librsvg \
  webkit2gtk-4.1 \
  gtk3 \
  gst-plugins-base \
  gst-plugins-good \
  rust \
  nodejs \
  npm

echo "==> Rust toolchain (stable)"
if command -v rustup >/dev/null 2>&1; then
  rustup default stable
  rustup target add x86_64-unknown-linux-gnu || true
fi

echo "==> npm install (frontend)"
cd "$(dirname "$0")/.."
npm install

echo "==> cargo fetch / build core + CLI + MCP"
cargo build -p moraine-core -p moraine-cli -p moraine-mcp

echo ""
echo "Done."
echo "  CLI:     cargo run -p moraine-cli -- info"
echo "  MCP:     cargo run -p moraine-cli -- mcp --project \$PWD"
echo "  Desktop: npm run tauri:dev"
echo "  Tests:   npm run test:rust"
echo "  MSRV:    Rust 1.88 (workspace rust-version)"
