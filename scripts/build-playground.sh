#!/usr/bin/env bash
# Build the WASM engine and stage the browser playground next to it. The docs
# deploy runs this so /playground/ ships the .wasm + JS glue alongside the page.
set -euo pipefail
cd "$(dirname "$0")/.."
wasm-pack build wasm --target web --out-dir pkg --release
mkdir -p site-playground
cp docs/playground.html site-playground/index.html
cp wasm/pkg/timeglyph_wasm.js wasm/pkg/timeglyph_wasm_bg.wasm site-playground/
echo "Playground staged in ./site-playground (serve it, open index.html)."
