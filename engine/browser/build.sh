#!/usr/bin/env bash
# Build the in-browser engine into engine/browser/dist/ (Trunk copies it to the web app's engine/).
#   dist/engine-worker.js        the worker: PHP compiled to WebAssembly + engine-core.js
#   dist/8_2_35/php_8_2.wasm     PHP 8.2 (asyncify build, every current browser)
#   dist/imathas-engine.zip      IMathAS's PHP the engine runs, plus engine/serve.php and momformat.php
#   dist/dad-engine.js           the page's client (window.dadEngine)
#   dist/imathas/                the sandbox page's scripts and styles (MyOpenMath's student view)
# IMathAS comes from the same pinned commit as engine/Dockerfile.
set -euo pipefail
cd "$(dirname "$0")"
IMATHAS_COMMIT=$(grep -oP 'IMATHAS_COMMIT=\K[0-9a-f]+' ../Dockerfile)
SRC="${IMATHAS_SRC:-$HOME/.cache/dad/IMathAS}"
if [ ! -d "$SRC/.git" ] || [ "$(git -C "$SRC" rev-parse HEAD 2>/dev/null)" != "$IMATHAS_COMMIT" ]; then
  rm -rf "$SRC"; git init -q "$SRC"
  git -C "$SRC" remote add origin https://github.com/drlippman/IMathAS.git
  git -C "$SRC" fetch -q --depth 1 origin "$IMATHAS_COMMIT"
  git -C "$SRC" checkout -q FETCH_HEAD
fi
[ -d node_modules ] || npm install --no-audit --no-fund >/dev/null
rm -rf dist && mkdir -p dist
bun build-worker.mjs | tail -5
WASM_DIR=$(ls -d node_modules/@php-wasm/web-8-2/asyncify/8_2_*)
mkdir -p "dist/$(basename "$WASM_DIR")"
cp "$WASM_DIR"/php_8_2.wasm "dist/$(basename "$WASM_DIR")/"
cp dad-engine.js dist/
STAGE=$(mktemp -d)
(cd "$SRC" && cp -r assess2 assessment includes filter header.php footer.php "$STAGE/")
cp ../serve.php ../momformat.php ../student.php "$STAGE/"
(cd "$STAGE" && zip -qr -9 "$OLDPWD/dist/imathas-engine.zip" .)
rm -rf "$STAGE"
# The sandbox page's scripts, styles, fonts and images (MyOpenMath's student view), served by the web
# app as static files: top-level scripts, MathJax 4, MathQuill, the assess2 styles, icons, images and
# the stylesheets at IMathAS's root. Not the editors or video player the student view never loads.
S=dist/imathas
mkdir -p "$S/javascript"
cp "$SRC"/javascript/*.js "$S/javascript/"
cp -r "$SRC/javascript/mathjax4" "$S/javascript/"
mkdir -p "$S/assess2"
cp -r "$SRC/mathquill" "$SRC/iconfonts" "$SRC/img" "$S/"
cp -r "$SRC/assess2/vue" "$S/assess2/"
cp "$SRC"/*.css "$S/"
echo "static assets: $(du -sh "$S" | cut -f1), $(find "$S" -type f | wc -l) files, largest $(find "$S" -type f -printf '%s\n' | sort -n | tail -1) bytes"
ls -la dist dist/8_2_* | grep -v '^total'
