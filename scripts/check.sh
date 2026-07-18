#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== fmt =="
cargo fmt --all -- --check

echo "== clippy =="
cargo clippy -p moraine-core -p moraine-cli -p moraine-server -- -D warnings

echo "== rust tests =="
cargo test -p moraine-core
cargo build -p moraine-server -q
cargo test -p moraine-cli

echo "== frontend =="
npm run check
npm test
npm run build

echo "ok"
