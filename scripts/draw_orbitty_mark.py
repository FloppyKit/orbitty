#!/usr/bin/env python3
"""Orbitty small mark — nest lock ORBITTY_MARK_AND_NAMES_v0.md + Matt 2026-08-23 pixel.

Tray (template): white silhouette, transparent field, ~25% fill.
Color preview / small app icons: near-black field, two-tone orbit, ochre square,
teal planet on the lower-right of the ring, cream figure ≈ square height.

Does not touch the 3D Dock icns.
"""
from __future__ import annotations

import math
import os
import struct
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
BRAND = os.path.join(ROOT, "brand")
ICONS = os.path.join(ROOT, "src-tauri", "icons")

# Matt palette
FIELD = (18, 16, 24)
LILAC = (186, 150, 186)
CREAM = (236, 228, 204)
OCHRE = (196, 140, 56)
TEAL = (64, 176, 168)
TEAL_D = (40, 128, 128)
FIGURE = (244, 236, 220)


def write_png(path: str, w: int, h: int, rgba: bytes) -> None:
    def chunk(tag: bytes, data: bytes) -> bytes:
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    raw = b"".join(b"\x00" + rgba[y * w * 4 : (y + 1) * w * 4] for y in range(h))
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(png)


def rot(x, y, ang):
    c, s = math.cos(ang), math.sin(ang)
    return x * c - y * s, x * s + y * c


def mark_coverage(u: float, v: float) -> str | None:
    """Unit square, y down. Returns layer id or None (field)."""
    # Matt photo + nest lock: high-LEFT → low-RIGHT (backslash).
    # y-down: +angle puts the right tip lower. Previous build used -28° (wrong).
    ang = math.radians(28)
    cx, cy = 0.48, 0.52
    rx, ry = 0.36, 0.155
    dx, dy = u - cx, v - cy
    lx, ly = rot(dx, dy, -ang)
    e = (lx / rx) ** 2 + (ly / ry) ** 2
    ring = abs(e - 1.0) * min(rx, ry)
    on_ring = ring < 0.050 and e > 0.70
    wx, wy = rot(dx, dy, -ang)

    # Planet rests on the lower-RIGHT of the ring (inside the canvas).
    px, py, pr = 0.68, 0.64, 0.095
    pd = math.hypot(u - px, v - py)
    on_planet = pd <= pr
    planet_hi = pd <= pr * 0.52 and (u - px) < 0 and (v - py) < 0

    # Ochre square: visual center of the ellipse, not on the planet.
    sq = 0.11
    sx, sy = 0.46, 0.46
    on_sq = (abs(u - sx) <= sq / 2) and (abs(v - sy) <= sq / 2)

    # Cream figure ON the planet, fully above the disc so nothing clips.
    # Height ≈ the ochre square. Head must stay inside the canvas.
    fh = sq * 1.05
    fx = px
    fbot = py - pr + 0.012  # feet kiss the crown
    ftop = fbot - fh
    hr = fh * 0.20
    hcx, hcy = fx, ftop + hr * 1.15
    on_head = math.hypot(u - hcx, v - hcy) <= hr
    tw, th = fh * 0.24, fh * 0.32
    tcx, tcy = fx, hcy + hr + th / 2
    on_torso = abs(u - tcx) <= tw / 2 and abs(v - tcy) <= th / 2
    aw, ah = fh * 0.70, fh * 0.11
    acy = tcy - th * 0.05
    on_arms = abs(u - fx) <= aw / 2 and abs(v - acy) <= ah / 2
    gap = fh * 0.12
    lw, lh = fh * 0.12, fh * 0.26
    ly0 = tcy + th / 2
    on_lleg = abs(u - (fx - gap)) <= lw / 2 and ly0 <= v <= min(ly0 + lh, fbot + 0.01)
    on_rleg = abs(u - (fx + gap)) <= lw / 2 and ly0 <= v <= min(ly0 + lh, fbot + 0.01)
    on_fig = on_head or on_torso or on_arms or on_lleg or on_rleg

    # Figure always wins (was hidden under the planet). Front (cream) orbit
    # crosses the ball; lilac stays behind.
    if on_fig:
        return "figure"
    if on_ring and wy >= 0.0:
        return "cream"
    if on_planet:
        return "planet_hi" if planet_hi else "planet"
    if on_sq:
        return "square"
    if on_ring:
        return "lilac"
    return None


def render(size: int, *, template: bool, color_field: bool) -> bytes:
    ss = 5 if size <= 64 else 3
    buf = bytearray(size * size * 4)
    filled = 0
    for py in range(size):
        for px in range(size):
            acc = [0, 0, 0, 0]
            for sy in range(ss):
                for sx in range(ss):
                    u = (px + (sx + 0.5) / ss) / size
                    v = (py + (sy + 0.5) / ss) / size
                    layer = mark_coverage(u, v)
                    if template:
                        if layer:
                            acc[0] += 255
                            acc[1] += 255
                            acc[2] += 255
                            acc[3] += 255
                    else:
                        if layer == "lilac":
                            rgb = LILAC
                        elif layer == "cream":
                            rgb = CREAM
                        elif layer == "square":
                            rgb = OCHRE
                        elif layer == "planet":
                            rgb = TEAL
                        elif layer == "planet_hi":
                            rgb = TEAL_D  # wait, highlight should be LIGHTER
                            rgb = (110, 210, 200)
                        elif layer == "figure":
                            rgb = FIGURE
                        else:
                            rgb = FIELD if color_field else None
                        if rgb is not None:
                            acc[0] += rgb[0]
                            acc[1] += rgb[1]
                            acc[2] += rgb[2]
                            acc[3] += 255 if (layer or color_field) else 0
                        elif color_field:
                            acc[0] += FIELD[0]
                            acc[1] += FIELD[1]
                            acc[2] += FIELD[2]
                            acc[3] += 255
            n = ss * ss
            i = (py * size + px) * 4
            buf[i] = acc[0] // n
            buf[i + 1] = acc[1] // n
            buf[i + 2] = acc[2] // n
            buf[i + 3] = acc[3] // n
            if buf[i + 3] > 80:
                filled += 1
    frac = filled / (size * size)
    print(f"  {size}px template={template} field={color_field} fill={filled}/{size*size} ({frac:.3f})")
    return bytes(buf)


def main() -> None:
    os.makedirs(BRAND, exist_ok=True)
    os.makedirs(ICONS, exist_ok=True)

    color176 = render(176, template=False, color_field=True)
    write_png(os.path.join(BRAND, "orbitty-mark-preview.png"), 176, 176, color176)

    color44 = render(44, template=False, color_field=True)
    write_png(os.path.join(BRAND, "orbitty-tray-44.png"), 44, 44, color44)
    write_png(os.path.join(BRAND, "orbitty-tray-preview.png"), 44, 44, color44)

    tmpl44 = render(44, template=True, color_field=False)
    write_png(os.path.join(BRAND, "orbitty-tray-template.png"), 44, 44, tmpl44)
    write_png(os.path.join(ICONS, "tray.png"), 44, 44, tmpl44)
    with open(os.path.join(ICONS, "tray.rgba"), "wb") as f:
        f.write(tmpl44)

    # Small app icons (not Dock 3D). Near-black field so they aren't a JPEG square.
    for name, sz in (("32x32.png", 32), ("Square44x44Logo.png", 44), ("Square30x30Logo.png", 30)):
        write_png(os.path.join(ICONS, name), sz, sz, render(sz, template=False, color_field=True))

    print("wrote brand/ + tray.rgba")


if __name__ == "__main__":
    main()
