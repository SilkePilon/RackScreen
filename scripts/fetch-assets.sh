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
OUTFIT="https://raw.githubusercontent.com/Outfitio/Outfit-Fonts/main"
for f in Outfit-Medium Outfit-SemiBold Outfit-Bold; do
  curl -fsSL "$OUTFIT/fonts/ttf/$f.ttf" -o "assets/fonts/$f.ttf"
  head -c 4 "assets/fonts/$f.ttf" | od -An -tx1 | grep -q "00 01 00 00" || { echo "bad ttf: $f"; exit 1; }
done
curl -fsSL "$OUTFIT/OFL.txt" -o assets/fonts/Outfit-LICENSE.txt
echo "assets ok"
