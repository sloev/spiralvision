#!/usr/bin/env bash
# Build + package Linux: tarball + AppImage. Usage: package-linux.sh <x86_64|aarch64>
# aarch64 is cross-compiled on x86_64 (needs scripts/apt-arm64.sh + gcc-aarch64-linux-gnu).
set -euo pipefail
cd "$(dirname "$0")/.."
ARCH=$1
# Build what we package (never rely on a previous build).
if [ "$ARCH" = "$(uname -m)" ]; then
  cargo build --release -p spiralvision -p spiralvision-cli
  BIN=target/release
else
  T=$ARCH-unknown-linux-gnu
  export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc PKG_CONFIG_ALLOW_CROSS=1 \
    PKG_CONFIG_PATH=/usr/lib/$ARCH-linux-gnu/pkgconfig PKG_CONFIG_SYSROOT_DIR=/
  cargo build --release --target $T -p spiralvision -p spiralvision-cli
  BIN=target/$T/release
fi
D=spiralvision-linux-$ARCH
rm -rf dist/$D AppDir && mkdir -p dist/$D
cp $BIN/spiralvision $BIN/spiralvision-cli README.md dist/$D/
cp .github/assets/icon-1024.png dist/$D/spiralvision.png
tar czf dist/$D.tar.gz -C dist $D

if [ "${SKIP_APPIMAGE:-}" = 1 ]; then ls -l dist/$D.tar.gz; exit 0; fi
mkdir -p AppDir/usr/bin AppDir/usr/share/icons/hicolor/256x256/apps
cp $BIN/spiralvision $BIN/spiralvision-cli AppDir/usr/bin/
python3 -c "from PIL import Image; Image.open('.github/assets/icon-1024.png').resize((256,256)).save('AppDir/spiralvision.png')" 2>/dev/null \
  || convert .github/assets/icon-1024.png -resize 256x256 AppDir/spiralvision.png
cp AppDir/spiralvision.png AppDir/.DirIcon
cp AppDir/spiralvision.png AppDir/usr/share/icons/hicolor/256x256/apps/spiralvision.png
printf '[Desktop Entry]\nName=SpiralVision\nExec=spiralvision\nIcon=spiralvision\nType=Application\nCategories=AudioVideo;\n' > AppDir/spiralvision.desktop
printf '#!/bin/sh\nHERE="$(dirname "$(readlink -f "$0")")"\nexport PATH="$HERE/usr/bin:$PATH"\nexec spiralvision "$@"\n' > AppDir/AppRun
chmod +x AppDir/AppRun
TOOL=dist/appimagetool-$(uname -m)
[ -x "$TOOL" ] || { curl -sSL -o "$TOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$(uname -m).AppImage"; chmod +x "$TOOL"; }
ARCH=$ARCH "$TOOL" --appimage-extract-and-run AppDir dist/SpiralVision-$ARCH.AppImage >/dev/null
rm -rf AppDir
ls -l dist/$D.tar.gz dist/SpiralVision-$ARCH.AppImage
