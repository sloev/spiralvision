#!/usr/bin/env bash
# Enable arm64 multiarch packages (for cross-compiling the Raspberry Pi build)
# on an Ubuntu 24.04 x86_64 host. Pins existing sources to amd64 and adds ports.
set -euo pipefail
dpkg --add-architecture arm64
for f in /etc/apt/sources.list.d/*.sources; do
  [ -f "$f" ] || continue
  grep -q '^Architectures:' "$f" || sed -i '/^Types:/a Architectures: amd64' "$f"
done
if [ -f /etc/apt/sources.list ]; then
  sed -i -E 's/^deb (http)/deb [arch=amd64] \1/' /etc/apt/sources.list
fi
for f in /etc/apt/sources.list.d/*.list; do
  [ -f "$f" ] || continue
  sed -i -E 's/^deb (http)/deb [arch=amd64] \1/' "$f"
done
printf 'Types: deb\nURIs: http://ports.ubuntu.com/ubuntu-ports/\nSuites: noble noble-updates\nComponents: main universe\nArchitectures: arm64\n' \
  > /etc/apt/sources.list.d/arm64-ports.sources
