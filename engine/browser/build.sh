#!/usr/bin/env bash
# Build the in-browser engine into engine/browser/dist/ (Trunk copies it to the web app's engine/).
#   dist/engine-worker.js        the worker: PHP compiled to WebAssembly + engine-core.js
#   dist/8_2_35/php_8_2.wasm     PHP 8.2 (asyncify build, every current browser)
#   dist/imathas-engine.zip      IMathAS's PHP the engine runs, plus engine/serve.php and momformat.php
#   dist/dad-engine.js           the page's client (window.dadEngine)
#   dist/imathas/                the sandbox page's scripts and styles (MyOpenMath's student view)
# IMathAS comes from the same pinned commit as engine/Dockerfile (checker-files.sh).
set -euo pipefail
cd "$(dirname "$0")"
[ -d node_modules ] || npm install --no-audit --no-fund >/dev/null
rm -rf dist && mkdir -p dist
bun build-worker.mjs | tail -5
WASM_DIR=$(ls -d node_modules/@php-wasm/web-8-2/asyncify/8_2_*)
mkdir -p "dist/$(basename "$WASM_DIR")"
cp "$WASM_DIR"/php_8_2.wasm "dist/$(basename "$WASM_DIR")/"
cp dad-engine.js dist/
./checker-files.sh dist
ls -la dist dist/8_2_* | grep -v '^total'
