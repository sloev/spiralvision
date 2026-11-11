#!/usr/bin/env bash
# Build the Android APK (web deck + WASM bundled). Needs ANDROID_HOME and gradle.
# Signs with $KEYSTORE_B64 (base64 .jks) when set, otherwise with a debug key.
set -euo pipefail
cd "$(dirname "$0")/.."
[ -f docs/wasm/spiralvision.wasm ] || ./scripts/build-wasm.sh
if [ -n "${KEYSTORE_B64:-}" ]; then
  echo "$KEYSTORE_B64" | base64 -d > "${RUNNER_TEMP:-/tmp}/release.jks"
  export ANDROID_KEYSTORE="${RUNNER_TEMP:-/tmp}/release.jks"
else
  # No release key configured: sign with a throwaway key kept in android/build/.
  KS=android/build/throwaway.jks
  mkdir -p android/build
  [ -f "$KS" ] || keytool -genkeypair -keystore "$KS" -storepass spiralvision -alias spiralvision \
    -keypass spiralvision -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=SpiralVision,O=SpiralVision,C=DK" >/dev/null 2>&1
  export ANDROID_KEYSTORE="$PWD/$KS" ANDROID_KEYSTORE_PASSWORD=spiralvision ANDROID_KEY_ALIAS=spiralvision ANDROID_KEY_PASSWORD=spiralvision
fi
gradle -p android --no-daemon -q assembleRelease
mkdir -p dist && cp android/app/build/outputs/apk/release/app-release.apk dist/SpiralVision-android.apk
ls -l dist/SpiralVision-android.apk
