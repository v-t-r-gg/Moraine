#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== fmt =="
cargo fmt --all -- --check

echo "== clippy =="
RUSTFLAGS="-D warnings" cargo clippy \
  -p moraine-platform \
  -p moraine-core \
  -p moraine-cli \
  -p moraine-mcp \
  -p moraine-server \
  -p moraine-service \
  -p moraine-provision \
  --all-targets -- -D warnings

echo "== rust tests =="
cargo test -p moraine-platform
cargo test -p moraine-core
cargo build -p moraine-server -q
cargo build -p moraine-cli -q
cargo test -p moraine-cli
cargo test -p moraine-mcp
cargo test -p moraine-service
cargo test -p moraine-provision

echo "== tauri =="
cargo check -p moraine-app
cargo test -p moraine-app --test provision_commands

echo "== frontend =="
npm run typecheck
npm test
npm run build

echo "ok"
