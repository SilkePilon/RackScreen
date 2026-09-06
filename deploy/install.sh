#!/usr/bin/env bash
# Usage: ./install.sh <path-to-rackscreen-binary> [user]
set -euo pipefail
BIN="${1:?binary path}"
USER_NAME="${2:-$USER}"
sudo install -m 755 "$BIN" /usr/local/bin/rackscreen
mkdir -p "$HOME/.config/rackscreen"
[ -f "$HOME/.config/rackscreen/config.toml" ] || cp "$(dirname "$0")/../config.example.toml" "$HOME/.config/rackscreen/config.toml"
sudo usermod -aG spi,gpio "$USER_NAME"
sudo install -m 644 "$(dirname "$0")/rackscreen.service" /etc/systemd/system/rackscreen@.service
sudo systemctl daemon-reload
sudo systemctl enable --now "rackscreen@${USER_NAME}"
echo "installed; logs: journalctl -u rackscreen@${USER_NAME} -f"
