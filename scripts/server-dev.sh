#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export RUST_LOG="${RUST_LOG:-info}"
export MORAINE_BIND="${MORAINE_BIND:-127.0.0.1:3099}"
exec cargo run -p moraine-server -- "$@"
