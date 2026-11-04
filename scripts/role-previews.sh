#!/usr/bin/env bash
# Turn the renderer's golden images into the web UI's role previews:
# crates/render/tests/goldens/p2_<role>.png -> web/public/roles/<role>.webp.
# Rerun after the goldens change. Needs ImageMagick.
set -euo pipefail
cd "$(dirname "$0")/.."
src=crates/render/tests/goldens
out=web/public/roles
mkdir -p "$out"
roles=(cpu mem pods health thermal storage power-mix renewable gh-activity weather wind aqi rain sun moon iss ups net deploys media traffic restarts uptime fans mains load ci jellyfin music)
for r in "${roles[@]}"; do
  png="$src/p2_$r.png"
  [[ -f $png ]] || { echo "no golden for $r: $png" >&2; exit 1; }
  magick "$png" -resize 160x160 -quality 82 -define webp:method=6 "$out/$r.webp"
done
du -ch "$out"/*.webp | tail -1
