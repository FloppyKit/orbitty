#!/usr/bin/env python3
"""Turn Matt's IcoB / IcoW JPEGs into 44×44 transparent tray RGBA.

IcoB = black mark (light menu bar). IcoW = white mark (dark menu bar).
JPEG backgrounds are knocked out. Not used as a macOS template (that
would flatten a dark JPEG into a white square).
"""
from __future__ import annotations

import os
import struct
import zlib
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
BRAND = os.path.join(ROOT, "brand")
ICONS = os.path.join(ROOT, "src-tauri", "icons")
SIZE = 44


def write_png(path: str, im: Image.Image) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    im.save(path, "PNG")


def write_rgba(path: str, im: Image.Image) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(im.convert("RGBA").tobytes())


def knock_out(path: str, keep_dark: bool) -> Image.Image:
    src = Image.open(path).convert("RGB")
    w, h = src.size
    px = src.load()
    out = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    opx = out.load()
    minx, miny, maxx, maxy = w, h, 0, 0
    for y in range(h):
        for x in range(w):
            r, g, b = px[x, y]
            lum = (r + g + b) / 3
            if keep_dark:
                # black ink on white paper
                if lum > 210:
                    continue
                # harden to black
                a = 255 if lum < 140 else int(255 * (210 - lum) / 70)
                if a < 40:
                    continue
                opx[x, y] = (0, 0, 0, 255 if a > 160 else a)
            else:
                # white ink on black paper
                if lum < 40:
                    continue
                a = 255 if lum > 180 else int(255 * (lum - 40) / 140)
                if a < 40:
                    continue
                opx[x, y] = (255, 255, 255, 255 if a > 160 else a)
            if x < minx:
                minx = x
            if y < miny:
                miny = y
            if x > maxx:
                maxx = x
            if y > maxy:
                maxy = y
    if maxx <= minx:
        return out
    pad = 24
    box = (
        max(0, minx - pad),
        max(0, miny - pad),
        min(w, maxx + 1 + pad),
        min(h, maxy + 1 + pad),
    )
    crop = out.crop(box)
    # Fit into SIZE×SIZE, nearest, keep aspect, centered.
    cw, ch = crop.size
    scale = min(SIZE / cw, SIZE / ch)
    nw, nh = max(1, int(cw * scale)), max(1, int(ch * scale))
    # Prefer integer-ish downsample for pixel art
    step = max(1, round(cw / SIZE))
    nw = max(1, cw // step)
    nh = max(1, ch // step)
    if nw > SIZE or nh > SIZE:
        crop = crop.resize((SIZE, SIZE), Image.Resampling.NEAREST)
        return crop
    small = crop.resize((nw, nh), Image.Resampling.NEAREST)
    canvas = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    canvas.paste(small, ((SIZE - nw) // 2, (SIZE - nh) // 2), small)
    return canvas


def fill_frac(im: Image.Image) -> float:
    a = list(im.getdata())
    n = sum(1 for p in a if p[3] > 80)
    return n / (im.size[0] * im.size[1])


def main() -> None:
    light = knock_out("/Users/dude/Downloads/IcoB.jpg", keep_dark=True)
    dark = knock_out("/Users/dude/Downloads/IcoW.jpg", keep_dark=False)
    print("light fill", round(fill_frac(light), 3), "dark fill", round(fill_frac(dark), 3))
    write_png(os.path.join(BRAND, "orbitty-tray-light.png"), light)
    write_png(os.path.join(BRAND, "orbitty-tray-dark.png"), dark)
    write_png(os.path.join(ICONS, "tray-light.png"), light)
    write_png(os.path.join(ICONS, "tray-dark.png"), dark)
    write_rgba(os.path.join(ICONS, "tray-light.rgba"), light)
    write_rgba(os.path.join(ICONS, "tray-dark.rgba"), dark)
    # default tray.rgba = dark (white mark) so a missed switch still reads
    # on the usual dark menu bar; light mode swaps in tray-light.
    write_rgba(os.path.join(ICONS, "tray.rgba"), dark)
    write_png(os.path.join(ICONS, "tray.png"), dark)
    print("wrote tray-light / tray-dark")


if __name__ == "__main__":
    main()
