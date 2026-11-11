#!/usr/bin/env bash
# Bundle universal SpiralVision.app (run on macOS after building both targets).
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION=${1:-0.2.0}
APP=dist/SpiralVision.app
rm -rf "$APP" dist/icon.iconset && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" dist/icon.iconset
for b in spiralvision spiralvision-cli; do
  lipo -create target/aarch64-apple-darwin/release/$b target/x86_64-apple-darwin/release/$b -output "$APP/Contents/MacOS/$b"
done
for s in 16 32 128 256 512; do
  sips -z $s $s .github/assets/icon-1024.png --out dist/icon.iconset/icon_${s}x${s}.png >/dev/null
  sips -z $((s*2)) $((s*2)) .github/assets/icon-1024.png --out dist/icon.iconset/icon_${s}x${s}@2x.png >/dev/null
done
iconutil -c icns dist/icon.iconset -o "$APP/Contents/Resources/SpiralVision.icns"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
codesign --force --deep -s - "$APP"
(cd dist && ditto -c -k --keepParent SpiralVision.app SpiralVision-macos-universal.zip)
ls -l dist/SpiralVision-macos-universal.zip
