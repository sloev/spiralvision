#!/usr/bin/env bash
# Build the Android APK (web deck + WASM bundled). Needs ANDROID_HOME and gradle.
# Signs with $KEYSTORE_B64 (base64 .jks) when set, otherwise with a debug key.
set -euo pipefail
cd "$(dirname "$0")/.."
[ -f docs/wasm/spiralvision.wasm ] || ./scripts/build-wasm.sh
if [ -n "${KEYSTORE_B64:-}" ]; then
  echo "$KEYSTORE_B64" | base64 -d > "${RUNNER_TEMP:-/tmp}/release.jks"
  export ANDROID_KEYSTORE="${RUNNER_TEMP:-/tmp}/release.jks"
elif [ ! -f ~/.android/debug.keystore ]; then
  mkdir -p ~/.android
  keytool -genkeypair -keystore ~/.android/debug.keystore -storepass android -alias androiddebugkey \
    -keypass android -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Android Debug,O=Android,C=US"
fi
gradle -p android --no-daemon -q assembleRelease
mkdir -p dist && cp android/app/build/outputs/apk/release/app-release.apk dist/SpiralVision-android.apk
ls -l dist/SpiralVision-android.apk
