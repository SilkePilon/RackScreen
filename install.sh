#!/usr/bin/env bash
# RackScreen installer: downloads the latest release binary and opens the setup TUI.
#   curl -fsSL https://github.com/silkepilon/RackScreen/releases/latest/download/install.sh | bash
set -euo pipefail
REPO="silkepilon/RackScreen"
ARCH="$(uname -m)"
if [ "$ARCH" != "aarch64" ]; then
  echo "RackScreen needs a 64-bit Raspberry Pi OS (aarch64); this machine is $ARCH" >&2
  exit 1
fi
URL="https://github.com/$REPO/releases/latest/download/rackscreen-aarch64"
TMP="$(mktemp -d)"
echo "downloading $URL"
curl -fsSL "$URL" -o "$TMP/rackscreen"
chmod +x "$TMP/rackscreen"
exec sudo "$TMP/rackscreen" setup < /dev/tty
