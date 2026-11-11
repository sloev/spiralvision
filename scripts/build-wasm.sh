#!/usr/bin/env bash
# Build the shared DSP core to WebAssembly for the web app / PWA / Android.
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build --release --target wasm32-unknown-unknown -p spiralvision-wasm
mkdir -p docs/wasm
cp target/wasm32-unknown-unknown/release/spiralvision_wasm.wasm docs/wasm/spiralvision.wasm
if command -v wasm-opt >/dev/null; then wasm-opt -O3 docs/wasm/spiralvision.wasm -o docs/wasm/spiralvision.wasm; fi
ls -l docs/wasm/spiralvision.wasm
