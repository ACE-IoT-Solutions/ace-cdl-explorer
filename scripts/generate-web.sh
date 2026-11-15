#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")/.."
# Build wasm first with scripts/build-wasm.sh. --check never rewrites committed output.
if [ "${1:-}" = '--check' ]; then
  cargo run --quiet -p ace-building-sim-tools --features ts --bin generate-types -- --check
  node scripts/generate-catalog.mjs --check
else
  cargo run --quiet -p ace-building-sim-tools --features ts --bin generate-types
  node scripts/generate-catalog.mjs
fi
