#!/usr/bin/env bash
# Portable desktop packages of D.A.D: nothing to install, no admin rights.
#   Linux:  dist/desktop/DAD-<version>-linux-<arch>.tar.gz  (DAD + engine/ in one folder)
#   macOS:  dist/desktop/DAD-<version>-macos-<arch>.zip     (DAD.app; engine in Contents/Resources)
# The question checker's files ship beside the app; PHP is fetched once on first use (see
# src/mom/localengine.rs). Put a file named `portable` next to DAD or DAD.app to keep data in DAD-data/.
# usage: tools/package.sh [rust-target-triple]     (default: this machine)
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET="${1:-}"
VERSION="$(git describe --tags --always --dirty 2>/dev/null || echo dev)"
OS="$(uname -s)"
case "${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}" in
  x86_64-unknown-linux-*) PLAT=linux-x86_64 ;;
  aarch64-unknown-linux-*) PLAT=linux-aarch64 ;;
  x86_64-apple-darwin) PLAT=macos-x86_64 ;;
  aarch64-apple-darwin) PLAT=macos-aarch64 ;;
  *) echo "unsupported target: ${TARGET:-host}" >&2; exit 2 ;;
esac

engine/browser/checker-files.sh target/checker-files
cargo build --release --locked --bin vershion ${TARGET:+--target "$TARGET"}
BIN="target/${TARGET:+$TARGET/}release/vershion"

NAME="DAD-$VERSION-$PLAT"
OUT="dist/desktop"
rm -rf "$OUT/$NAME" && mkdir -p "$OUT/$NAME"
README="$OUT/$NAME/README.txt"

if [ "$OS" = Darwin ]; then
  APP="$OUT/$NAME/DAD.app"
  mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
  cp "$BIN" "$APP/Contents/MacOS/DAD"
  cp -R target/checker-files "$APP/Contents/Resources/engine"
  # The icon, from the 512 px PNG (iconutil wants a .iconset of sizes).
  ICONSET="$(mktemp -d)/DAD.iconset"; mkdir -p "$ICONSET"
  for s in 16 32 128 256 512; do
    sips -z $s $s assets/icons/icon-512.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2)); [ $d -le 512 ] && sips -z $d $d assets/icons/icon-512.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/DAD.icns"
  cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>DAD</string>
  <key>CFBundleDisplayName</key><string>D.A.D</string>
  <key>CFBundleIdentifier</key><string>com.shuff57.dad</string>
  <key>CFBundleExecutable</key><string>DAD</string>
  <key>CFBundleIconFile</key><string>DAD</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION#v}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
  LICENSES="$APP/Contents/Resources"
else
  cp "$BIN" "$OUT/$NAME/DAD"
  cp -r target/checker-files "$OUT/$NAME/engine"
  cp assets/icons/icon-512.png "$OUT/$NAME/DAD.png"
  LICENSES="$OUT/$NAME"
fi
cp LICENSE "$LICENSES/LICENSE.txt"
cp engine/LICENSE.md "$LICENSES/ENGINE-LICENSE.md"

cat > "$README" <<TXT
D.A.D $VERSION ($PLAT)

Writes MyOpenMath questions with an AI, checks them in a built-in copy of IMathAS, lets you try
them as a student, and adds them to MyOpenMath for you.

Start: $( [ "$OS" = Darwin ] && echo "open DAD.app (the first time: right-click it, choose Open, then Open)" || echo "run ./DAD" )
Nothing to install. The first question check downloads the checker's PHP once (about 12 MB).
Keep all your data next to the app (USB stick, locked-down computer): create an empty file named
"portable" next to $( [ "$OS" = Darwin ] && echo "DAD.app" || echo "DAD" ).

The app is MIT-licensed; the IMathAS question checker it runs is GPL-2.0-only (see the licenses).
TXT

if [ "$OS" = Darwin ]; then
  # Seal the complete bundle, including licenses. Developer ID signing: docs/dad/SIGNING.md.
  codesign --force -s - "$APP"
  codesign --verify --deep --strict "$APP"
fi

cd "$OUT"
if [ "$OS" = Darwin ]; then
  ditto -c -k --keepParent "$NAME" "$NAME.zip"; echo "$OUT/$NAME.zip"
else
  tar czf "$NAME.tar.gz" "$NAME"; echo "$OUT/$NAME.tar.gz"
fi
