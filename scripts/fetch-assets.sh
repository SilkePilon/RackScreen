#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p assets/icons assets/fonts
ICONS="arrow-up"
for i in $ICONS; do
  curl -sSL "https://unpkg.com/lucide-static@0.544.0/icons/$i.svg" -o "assets/icons/$i.svg"
  grep -q "<svg" "assets/icons/$i.svg" || { echo "bad svg: $i"; exit 1; }
done
curl -sSL "https://raw.githubusercontent.com/lucide-icons/lucide/main/LICENSE" -o assets/icons/LICENSE
# Material Symbols (Rounded, weight 700) for the icon at the top of each screen.
# The em-* icons (and EM-LICENSE.md) are vendored by hand, not fetched here.
MATERIAL="https://unpkg.com/@material-symbols/svg-700@0.23.0/rounded"
MS_ICONS="air aq battery_charging_full bolt build co2 commit dark_mode deployed_code eco
  electrical_services euro hard_drive lan memory_alt memory mode_fan monitor_heart movie
  partly_cloudy_day rainy restart_alt rocket_launch satellite_alt speed swap_vert sync
  thermostat timer video_library wb_sunny"
for i in $MS_ICONS; do
  curl -fsSL "$MATERIAL/$i.svg" | sed 's/width="48" height="48"/width="24" height="24"/' > "assets/icons/ms-$i.svg"
  grep -q '<svg[^>]*width="24" height="24"' "assets/icons/ms-$i.svg" || { echo "bad svg: ms-$i"; exit 1; }
done
curl -fsSL "https://raw.githubusercontent.com/google/material-design-icons/master/LICENSE" -o assets/icons/MATERIAL-LICENSE.txt
OUTFIT="https://raw.githubusercontent.com/Outfitio/Outfit-Fonts/main"
for f in Outfit-Medium Outfit-SemiBold Outfit-Bold; do
  curl -fsSL "$OUTFIT/fonts/ttf/$f.ttf" -o "assets/fonts/$f.ttf"
  head -c 4 "assets/fonts/$f.ttf" | od -An -tx1 | grep -q "00 01 00 00" || { echo "bad ttf: $f"; exit 1; }
done
curl -fsSL "$OUTFIT/OFL.txt" -o assets/fonts/Outfit-LICENSE.txt
echo "assets ok"
