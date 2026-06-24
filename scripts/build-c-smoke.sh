#!/usr/bin/env bash
# Build the C smoke program against the freshly-built libpheno_bridge.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${ROOT}/target/release"
SRC="${ROOT}/c/examples/smoke.c"
OUT="${ROOT}/target/release/pheno_bridge_smoke"

cd "${ROOT}"
cargo build --release

cc -O2 -Wall -Wextra -o "${OUT}" "${SRC}" -L"${TARGET}" -lpheno_bridge
echo "Built: ${OUT}"