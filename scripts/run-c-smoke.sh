#!/usr/bin/env bash
# Run the C smoke program against libpheno_bridge.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${ROOT}/target/release"
BIN="${ROOT}/target/release/pheno_bridge_smoke"

if [[ "$(uname)" == "Darwin" ]]; then
  DYLD_LIBRARY_PATH="${TARGET}" "${BIN}"
else
  LD_LIBRARY_PATH="${TARGET}" "${BIN}"
fi