#!/usr/bin/env python3
"""Generate the Orbitty rail-glyph icon set (S2).

Mark: a vertical rail bar (accent) + three stacked row bars — the rail column
in miniature. App icons sit on a dark rounded tile; the tray variant is pure
monochrome alpha for use as a macOS template image.

Pure stdlib: writes PNG, ICO (PNG entries), ICNS (PNG entries), and a raw
RGBA file consumed by the Rust tray via include_bytes!.
"""

import os
import struct
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ICONS = os.path.join(HERE, "..", "src-tauri", "icons")

TILE = (16, 16, 20)       # #101014
ACCENT = (255, 176, 0)    # #FFB000
ROW = (232, 230, 225)     # #E8E6E1


def rounded_rect_cov(x, y, x0, y0, x1, y1, r):
    """Coverage-ish (inside test with SDF) of a rounded rect at point (x, y)."""
    cx = min(max(x, x0 + r), x1 - r)
    cy = min(max(y, y0 + r), y1 - r)
    dx, dy = x - cx, y - cy
    return dx * dx + dy * dy <= r * r or (x0 <= x <= x1 and y0 <= y <= y1)


def rect_hit(x, y, x0, y0, x1, y1):
    return x0 <= x <= x1 and y0 <= y <= y1


def glyph_shapes(tray):
    """Shape list: (kind, coords, color). coords in unit square, y down."""
    shapes = []
    if not tray:
        shapes.append(("tile", (0.0, 0.0, 1.0, 1.0, 0.225), TILE))
    # vertical rail (accent)
    shapes.append(("bar", (0.20, 0.18, 0.32, 0.82), ACCENT if not tray else (0, 0, 0)))
    # three stacked rows, varied widths like window titles
    rows = [(0.30, 0.80), (0.50, 0.68), (0.70, 0.74)]
    for cy, x1 in rows:
        shapes.append(("bar", (0.42, cy - 0.055, x1, cy + 0.055), ROW if not tray else (0, 0, 0)))
    return shapes


def render(size, tray=False):
    ss = 4 if size <= 256 else 2
    shapes = glyph_shapes(tray)
    buf = bytearray(size * size * 4)
    for py in range(size):
        for px in range(size):
            acc = [0, 0, 0, 0]
            for sy in range(ss):
                for sx in range(ss):
                    x = (px + (sx + 0.5) / ss) / size
                    y = (py + (sy + 0.5) / ss) / size
                    r = g = b = a = 0
                    for kind, c, col in shapes:
                        if kind == "tile":
                            x0, y0, x1, y1, rad = c
                            hit = rounded_rect_cov(x, y, x0, y0, x1, y1, rad)
                        else:
                            hit = rect_hit(x, y, *c)
                        if hit:
                            r, g, b = col
                            a = 255
                    acc[0] += r
                    acc[1] += g
                    acc[2] += b
                    acc[3] += a
            n = ss * ss
            i = (py * size + px) * 4
            buf[i] = acc[0] // n
            buf[i + 1] = acc[1] // n
            buf[i + 2] = acc[2] // n
            buf[i + 3] = acc[3] // n
    return bytes(buf)


def write_png(path, size, rgba):
    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    raw = b"".join(
        b"\x00" + rgba[y * size * 4 : (y + 1) * size * 4] for y in range(size)
    )
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )
    with open(path, "wb") as f:
        f.write(png)
    return png


def write_ico(path, pngs):
    # pngs: list of (size, png_bytes); 256 stored as PNG, size byte 0
    header = struct.pack("<HHH", 0, 1, len(pngs))
    entries = b""
    offset = 6 + 16 * len(pngs)
    blob = b""
    for size, data in pngs:
        bsize = 0 if size >= 256 else size
        entries += struct.pack(
            "<BBBBHHII", bsize, bsize, 0, 0, 1, 32, len(data), offset
        )
        blob += data
        offset += len(data)
    with open(path, "wb") as f:
        f.write(header + entries + blob)


def write_icns(path, pngs_by_type):
    blob = b""
    for otype, data in pngs_by_type:
        blob += otype + struct.pack(">I", len(data) + 8) + data
    with open(path, "wb") as f:
        f.write(b"icns" + struct.pack(">I", len(blob) + 8) + blob)


def main():
    os.makedirs(ICONS, exist_ok=True)
    cache = {}

    def png_for(size, tray=False):
        key = (size, tray)
        if key not in cache:
            cache[key] = render(size, tray=tray)
        return cache[key]

    def png_bytes(size, tray=False):
        out = os.path.join(ICONS, f".tmp-{size}{'-tray' if tray else ''}.png")
        return write_png(out, size, png_for(size, tray)), out

    # --- standard PNGs (app icon, on tile) ---
    targets = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
        "Square30x30Logo.png": 30,
        "Square44x44Logo.png": 44,
        "Square71x71Logo.png": 71,
        "Square89x89Logo.png": 89,
        "Square107x107Logo.png": 107,
        "Square142x142Logo.png": 142,
        "Square150x150Logo.png": 150,
        "Square284x284Logo.png": 284,
        "Square310x310Logo.png": 310,
        "StoreLogo.png": 50,
    }
    for name, size in targets.items():
        write_png(os.path.join(ICONS, name), size, png_for(size))
        print(f"png  {name} ({size})")

    # --- tray: monochrome glyph, no tile ---
    write_png(os.path.join(ICONS, "tray.png"), 44, png_for(44, tray=True))
    with open(os.path.join(ICONS, "tray.rgba"), "wb") as f:
        f.write(png_for(44, tray=True))
    print("png  tray.png (44, template) + tray.rgba")

    # --- ICO (Windows): 16/32/48 classic + 256 PNG ---
    ico_pngs = []
    for size in (16, 32, 48, 256):
        data, tmp = png_bytes(size)
        ico_pngs.append((size, data))
        os.remove(tmp)
    write_ico(os.path.join(ICONS, "icon.ico"), ico_pngs)
    print("ico  icon.ico (16/32/48/256)")

    # --- ICNS (macOS): PNG payloads per OSType ---
    icns_types = [
        (b"icp4", 16),
        (b"icp5", 32),
        (b"icp6", 64),
        (b"ic07", 128),
        (b"ic08", 256),
        (b"ic09", 512),
        (b"ic10", 1024),
    ]
    entries = []
    for otype, size in icns_types:
        data, tmp = png_bytes(size)
        entries.append((otype, data))
        os.remove(tmp)
    write_icns(os.path.join(ICONS, "icon.icns"), entries)
    print("icns icon.icns (16..1024)")


if __name__ == "__main__":
    main()
