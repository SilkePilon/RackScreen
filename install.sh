#!/usr/bin/env bash
# RackScreen installer: downloads the latest release binary and installs the service.
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
trap 'rm -rf "$TMP"' EXIT
echo "downloading $URL"
curl -fsSL "$URL" -o "$TMP/rackscreen"
chmod +x "$TMP/rackscreen"
# The install asks y/N for the SPI boot lines on a terminal. Piped from curl
# stdin is the script, so ask on /dev/tty; without any terminal (ssh -T,
# automation) apply them, the change is idempotent and needed for the panels.
# /dev/tty exists and looks readable even without a controlling terminal, so
# probe by actually opening it.
if [ -t 0 ]; then
  sudo "$TMP/rackscreen" install
elif (exec </dev/tty) 2>/dev/null; then
  sudo "$TMP/rackscreen" install < /dev/tty
else
  sudo "$TMP/rackscreen" install --yes
fi
