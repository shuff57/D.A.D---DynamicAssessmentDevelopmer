#!/usr/bin/env bash
# The question checker's files, for the web app (engine/browser/build.sh) and the desktop app
# (tools/package.sh): needs only git and zip.
#   <out>/imathas-engine.zip   IMathAS's PHP from the pinned commit, plus engine/*.php
#   <out>/imathas/             the student page's scripts, styles, fonts and images
# usage: engine/browser/checker-files.sh <out-dir>
set -euo pipefail
OUT="$(mkdir -p "$1" && cd "$1" && pwd)"
cd "$(dirname "$0")"
IMATHAS_COMMIT=$(grep -oE 'IMATHAS_COMMIT=[0-9a-f]+' ../Dockerfile | cut -d= -f2)
SRC="${IMATHAS_SRC:-$HOME/.cache/dad/IMathAS}"
if [ ! -d "$SRC/.git" ] || [ "$(git -C "$SRC" rev-parse HEAD 2>/dev/null)" != "$IMATHAS_COMMIT" ]; then
  rm -rf "$SRC"; git init -q "$SRC"
  git -C "$SRC" remote add origin https://github.com/drlippman/IMathAS.git
  git -C "$SRC" fetch -q --depth 1 origin "$IMATHAS_COMMIT"
  git -C "$SRC" checkout -q FETCH_HEAD
fi
rm -rf "$OUT/imathas-engine.zip" "$OUT/imathas"
STAGE=$(mktemp -d)
(cd "$SRC" && cp -r assess2 assessment includes filter header.php footer.php "$STAGE/")
cp ../serve.php ../momformat.php ../student.php ../assess.php "$STAGE/"
(cd "$STAGE" && zip -qr -9 "$OUT/imathas-engine.zip" .)
rm -rf "$STAGE"
# Not the editors or video player the student view never loads.
S="$OUT/imathas"
mkdir -p "$S/javascript" "$S/assess2"
cp "$SRC"/javascript/*.js "$S/javascript/"
cp -r "$SRC/javascript/mathjax4" "$S/javascript/"
cp -r "$SRC/mathquill" "$SRC/iconfonts" "$SRC/img" "$S/"
cp -r "$SRC/assess2/vue" "$S/assess2/"
cp "$SRC"/*.css "$S/"
cp "$SRC/license.txt" "$OUT/IMathAS-LICENSE.txt"
echo "checker files: $(du -sh "$OUT/imathas-engine.zip" | cut -f1) PHP, $(du -sh "$S" | cut -f1) page files ($(find "$S" -type f | wc -l))"
