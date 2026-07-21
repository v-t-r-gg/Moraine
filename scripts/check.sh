#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== fmt =="
cargo fmt --all -- --check

echo "== clippy =="
cargo clippy -p moraine-core -p moraine-cli -p moraine-mcp -p moraine-server -- -D warnings

echo "== rust tests =="
cargo test -p moraine-core
cargo build -p moraine-server -q
cargo build -p moraine-cli -q
cargo test -p moraine-cli
cargo test -p moraine-mcp

echo "== frontend =="
npm run typecheck
npm test
npm run build

echo "ok"
