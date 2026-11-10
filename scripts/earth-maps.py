#!/usr/bin/env python3
"""Build the ISS globe's Earth maps from NASA imagery (public domain).

Inputs (downloaded into a temp dir): Blue Marble colours, the matching water
mask and Black Marble city lights, as packaged with the three-globe examples.

Outputs:
  assets/earth/classes.rle  1024x512 class indices (0 ocean, 1 forest,
                            2 grass, 3 desert, 4 ice), cleaned with a
                            majority filter so no single-texel specks flicker
                            as the globe turns; run-length coded as
                            (run 1..255, class) byte pairs, row-major.
  assets/earth/lights.bin   512x256 u8 night-lights strength (0..255).

Needs numpy and pillow:  python3 scripts/earth-maps.py
"""

import io
import pathlib
import sys
import tempfile
import urllib.request

import numpy as np
from PIL import Image

BASE = "https://cdn.jsdelivr.net/npm/three-globe@2.31.0/example/img/"
OUT = pathlib.Path(__file__).resolve().parent.parent / "assets" / "earth"
W, H = 2048, 1024


def fetch(name, tmp):
    path = pathlib.Path(tmp) / name
    if not path.exists():
        with urllib.request.urlopen(BASE + name, timeout=60) as r:
            path.write_bytes(r.read())
    return Image.open(path).convert("RGB").resize((W, H), Image.LANCZOS)


def box_sum(a, r):
    """Sum over a (2r+1)^2 box; wraps in x, clamps in y."""
    a = np.pad(a, ((r, r), (0, 0)), mode="edge")
    a = np.concatenate([a[:, -r:], a, a[:, :r]], axis=1)
    c = np.cumsum(np.cumsum(a, axis=0), axis=1)
    c = np.pad(c, ((1, 0), (1, 0)))
    k = 2 * r + 1
    return c[k:, k:] - c[:-k, k:] - c[k:, :-k] + c[:-k, :-k]


def majority(cls, r, n=5):
    sums = np.stack([box_sum((cls == k).astype(np.int32), r) for k in range(n)])
    return sums.argmax(axis=0).astype(np.uint8)


def main():
    with tempfile.TemporaryDirectory() as tmp:
        day = np.asarray(fetch("earth-blue-marble.jpg", tmp), dtype=np.float32)
        water = np.asarray(fetch("earth-water.png", tmp), dtype=np.float32)[..., 0] / 255
        night = np.asarray(fetch("earth-night.jpg", tmp), dtype=np.float32)

    r, g, b = day[..., 0], day[..., 1], day[..., 2]
    lum = 0.3 * r + 0.59 * g + 0.11 * b
    cls = np.full((H, W), 1, np.uint8)  # forest
    cls[lum > 62] = 2  # grass
    cls[(r > g * 1.06) & (lum > 100)] = 3  # desert
    cls[(lum > 175) & (np.abs(r - b) < 40)] = 4  # ice
    cls[water > 0.5] = 0  # ocean
    for _ in range(2):
        cls = majority(cls, 3)
    # 2048 -> 1024: majority of each 2x2 block
    blocks = cls.reshape(H // 2, 2, W // 2, 2).transpose(0, 2, 1, 3).reshape(H // 2, W // 2, 4)
    counts = np.stack([(blocks == k).sum(axis=2) for k in range(5)])
    small = counts.argmax(axis=0).astype(np.uint8)

    flat = small.ravel()
    out = bytearray()
    i = 0
    while i < len(flat):
        v = flat[i]
        j = i + 1
        while j < len(flat) and flat[j] == v and j - i < 255:
            j += 1
        out += bytes((j - i, v))
        i = j

    nl = (0.3 * night[..., 0] + 0.59 * night[..., 1] + 0.11 * night[..., 2]) / 255
    lights = np.clip((nl - 0.12) * 2.2, 0, 1)
    lights = box_sum(lights, 1) / 9.0
    lights = lights.reshape(H // 4, 4, W // 4, 4).mean(axis=(1, 3))

    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "classes.rle").write_bytes(bytes(out))
    (OUT / "lights.bin").write_bytes((lights * 255).round().astype(np.uint8).tobytes())
    print(f"classes.rle {len(out)} bytes, lights.bin {lights.size} bytes", file=sys.stderr)


if __name__ == "__main__":
    main()
