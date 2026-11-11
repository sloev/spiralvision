#!/usr/bin/env bash
# Everything the CI "test" job does — run this locally before tagging a release
# so GitHub Actions minutes are only spent on packaging.
set -euo pipefail
cd "$(dirname "$0")/.."
echo "== fmt";        cargo fmt --all -- --check
echo "== tests";      cargo test --workspace --release
echo "== wasm";       ./scripts/build-wasm.sh
echo "== wasm smoke"; node scripts/wasm-smoke.mjs
echo "== cli build";  cargo build --release -p spiralvision-cli
echo "== cli bench";  ./target/release/spiralvision-cli bench -f lavfi testsrc2=s=320x240:r=10 -t 3 --tape 1
if command -v actionlint >/dev/null; then echo "== actionlint"; actionlint; fi
echo "ALL GREEN. TOTALLY TUBULAR."
