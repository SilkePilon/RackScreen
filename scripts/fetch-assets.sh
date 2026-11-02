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
TMP=$(mktemp -d)
curl -sSL "https://github.com/rsms/inter/releases/download/v4.0/Inter-4.0.zip" -o "$TMP/inter.zip"
echo "ff970a5d4561a04f102a7cb781adbd6ac4e9b6c460914c7a101f15acb7f7d1a4  $TMP/inter.zip" | sha256sum -c -
unzip -q -o "$TMP/inter.zip" extras/ttf/Inter-Medium.ttf extras/ttf/Inter-SemiBold.ttf extras/ttf/Inter-Bold.ttf LICENSE.txt -d "$TMP"
cp "$TMP"/extras/ttf/Inter-Medium.ttf "$TMP"/extras/ttf/Inter-SemiBold.ttf "$TMP"/extras/ttf/Inter-Bold.ttf assets/fonts/
cp "$TMP/LICENSE.txt" assets/fonts/Inter-LICENSE.txt
rm -rf "$TMP"
echo "assets ok"
