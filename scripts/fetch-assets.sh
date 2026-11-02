#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p assets/icons assets/fonts
ICONS="arrow-up cpu memory-stick box heart-pulse download package-plus package-x package-minus flame circle-check server-off server triangle-alert shield-check plug-zap plug cloud-off euro cloud leaf thermometer database database-zap key-round zap smile"
for i in $ICONS; do
  curl -sSL "https://unpkg.com/lucide-static@0.544.0/icons/$i.svg" -o "assets/icons/$i.svg"
  grep -q "<svg" "assets/icons/$i.svg" || { echo "bad svg: $i"; exit 1; }
done
curl -sSL "https://raw.githubusercontent.com/lucide-icons/lucide/main/LICENSE" -o assets/icons/LICENSE
TMP=$(mktemp -d)
curl -sSL "https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip" -o "$TMP/jb.zip"
unzip -q -o "$TMP/jb.zip" -d "$TMP"
cp "$TMP/fonts/ttf/JetBrainsMono-Bold.ttf" assets/fonts/JetBrainsMono-Bold.ttf
cp "$TMP/OFL.txt" assets/fonts/OFL.txt
curl -sSL "https://github.com/rsms/inter/releases/download/v4.0/Inter-4.0.zip" -o "$TMP/inter.zip"
echo "ff970a5d4561a04f102a7cb781adbd6ac4e9b6c460914c7a101f15acb7f7d1a4  $TMP/inter.zip" | sha256sum -c -
unzip -q -o "$TMP/inter.zip" extras/ttf/Inter-Medium.ttf extras/ttf/Inter-SemiBold.ttf extras/ttf/Inter-Bold.ttf LICENSE.txt -d "$TMP"
cp "$TMP"/extras/ttf/Inter-Medium.ttf "$TMP"/extras/ttf/Inter-SemiBold.ttf "$TMP"/extras/ttf/Inter-Bold.ttf assets/fonts/
cp "$TMP/LICENSE.txt" assets/fonts/Inter-LICENSE.txt
rm -rf "$TMP"
echo "assets ok"
