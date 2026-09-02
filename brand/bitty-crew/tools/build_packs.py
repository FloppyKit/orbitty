#!/usr/bin/env python3
"""GO-ORBITTY-S1-BITTYS — build the Season 1 bitty packs from the blessed
planet-sprites drop.

Reads:  brand/bitty-crew/New Crew/planet-sprites/out/<kind>_walk[_<pal>]_v1.png
        (128x32 sheets, 4x 32x32 cells, 8-bit RGBA, hard alpha)
Writes: brand/bitty-crew/packs/orbitty-bitty-<kind>[-<pal>]/
        manifest.json + walk_0..3.png + idle.png

Facing lock: pack frames ship facing RIGHT (habitat flips on roam.dx < 0).
The blessed human sheets face left, so each 32x32 cell is mirrored
independently here. Animals already face right — copied, never redrawn.

Pure stdlib (zlib PNG), same convention as scripts/gen_icons.py.
Offline tool — nothing here ships into the .app but the packs/ output.
"""

import json
import os
import shutil
import struct
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "New Crew", "planet-sprites", "out"))
PACKS = os.path.normpath(os.path.join(HERE, "..", "packs"))

# kind stem, display name, palette tokens (default v1 first is implicit),
# human = faces left in out/ (flip); animal = already faces right.
KINDS = [
    ("boy", "Boy", ["green", "red", "tan"], True),
    ("girl", "Girl", ["blue", "brown", "green"], True),
    ("man", "Man", ["brown", "green", "red"], True),
    ("woman", "Woman", ["blue", "green", "red"], True),
    ("old_man", "Old Man", ["blue", "brown", "burgundy"], True),
    ("old_woman", "Old Woman", ["blue", "brown", "green"], True),
    ("cat", "Cat", ["black", "calico", "gray"], False),
    ("dog", "Dog", ["black", "spot", "white"], False),
    ("cow", "Cow", ["black", "brown", "white"], False),
    ("horse", "Horse", ["black", "palomino", "white"], False),
    ("chicken", "Chicken", ["black", "brown", "white"], False),
]

LICENSE = "Orbitty Season 1 bitty crew (Grok Imagine + Matt bible, blessed 2026-08-23)"


# ---------- minimal PNG codec (8-bit RGBA, color type 6, non-interlaced) ----------

def read_png(path):
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path}: not a PNG")
    off = 8
    width = height = None
    idat = bytearray()
    while off < len(data):
        ln, typ = struct.unpack(">I4s", data[off : off + 8])
        body = data[off + 8 : off + 8 + ln]
        if typ == b"IHDR":
            w, h, bd, ct, _comp, _filt, inter = struct.unpack(">IIBBBBB", body)
            if bd != 8 or ct != 6 or inter != 0:
                raise ValueError(f"{path}: want 8-bit RGBA non-interlaced, got bd={bd} ct={ct} inter={inter}")
            width, height = w, h
        elif typ == b"IDAT":
            idat += body
        elif typ == b"IEND":
            break
        off += 12 + ln
    if width is None:
        raise ValueError(f"{path}: no IHDR")
    raw = zlib.decompress(bytes(idat))
    stride = width * 4
    px = bytearray(width * height * 4)
    prev = bytearray(stride)
    pos = 0
    for y in range(height):
        filt = raw[pos]
        pos += 1
        row = bytearray(raw[pos : pos + stride])
        pos += stride
        if filt == 1:  # Sub
            for i in range(4, stride):
                row[i] = (row[i] + row[i - 4]) & 0xFF
        elif filt == 2:  # Up
            for i in range(stride):
                row[i] = (row[i] + prev[i]) & 0xFF
        elif filt == 3:  # Average
            for i in range(stride):
                a = row[i - 4] if i >= 4 else 0
                row[i] = (row[i] + ((a + prev[i]) >> 1)) & 0xFF
        elif filt == 4:  # Paeth
            for i in range(stride):
                a = row[i - 4] if i >= 4 else 0
                b = prev[i]
                c = prev[i - 4] if i >= 4 else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                row[i] = (row[i] + pr) & 0xFF
        elif filt != 0:
            raise ValueError(f"{path}: bad filter {filt}")
        px[y * stride : (y + 1) * stride] = row
        prev = row
    return width, height, bytes(px)


def write_png(path, width, height, px):
    def chunk(typ, body):
        c = struct.pack(">I", len(body)) + typ + body
        return c + struct.pack(">I", zlib.crc32(typ + body) & 0xFFFFFFFF)

    raw = bytearray()
    stride = width * 4
    for y in range(height):
        raw.append(0)
        raw += px[y * stride : (y + 1) * stride]
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    data = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )
    with open(path, "wb") as f:
        f.write(data)


# ---------- pixel ops (never redraw — only mirror / cut blessed pixels) ----------

def cell(px, sheet_w, col, size=32):
    out = bytearray(size * size * 4)
    for y in range(size):
        src = ((y * sheet_w) + col * size) * 4
        out[y * size * 4 : (y + 1) * size * 4] = px[src : src + size * 4]
    return bytes(out)


def hflip(px, size=32):
    out = bytearray(size * size * 4)
    for y in range(size):
        for x in range(size):
            dst = (y * size + x) * 4
            src = (y * size + (size - 1 - x)) * 4
            out[dst : dst + 4] = px[src : src + 4]
    return bytes(out)


def build_pack(kind, display, token, human):
    """One pack dir. token None = default v1."""
    suffix = f"_walk_{token}_v1.png" if token else "_walk_v1.png"
    src = os.path.join(OUT, f"{kind}{suffix}")
    w, h, px = read_png(src)
    if (w, h) != (128, 32):
        raise ValueError(f"{src}: want 128x32, got {w}x{h}")

    slug_kind = kind.replace("_", "-")
    slug = f"orbitty-bitty-{slug_kind}" + (f"-{token}" if token else "")
    dest = os.path.join(PACKS, slug)
    if os.path.isdir(dest):
        shutil.rmtree(dest)
    os.makedirs(dest)

    frames = []
    for i in range(4):
        c = cell(px, w, i)
        if human:
            c = hflip(c)
        name = f"walk_{i}.png"
        write_png(os.path.join(dest, name), 32, 32, c)
        frames.append(c)

    # Idle: the blessed still (base_32), same facing rule; absent art falls
    # back to walk frame 0 engine-side, so a missing base is not fatal.
    idle_names = []
    base = os.path.join(OUT, f"{kind}_base_32.png")
    if token is None and os.path.isfile(base):
        bw, bh, bpx = read_png(base)
        if (bw, bh) == (32, 32):
            if human:
                bpx = hflip(bpx)
            write_png(os.path.join(dest, "idle.png"), 32, 32, bpx)
            idle_names.append("idle.png")

    name = display if token is None else f"{display} — {token.capitalize()}"
    manifest = {
        "name": name,
        "license": LICENSE,
        "frames": [f"walk_{i}.png" for i in range(4)],
        "role": "walker",
    }
    if idle_names:
        manifest["idleFrames"] = idle_names
    with open(os.path.join(dest, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")

    # Self-check: re-decode walk_0 and confirm it is exactly the (maybe
    # mirrored) blessed cell 0 — no resample, no re-key, no redraw.
    rw, rh, rpx = read_png(os.path.join(dest, "walk_0.png"))
    want = cell(px, w, 0)
    if human:
        want = hflip(want)
    assert (rw, rh) == (32, 32) and rpx == want, f"{slug}: walk_0 round-trip mismatch"
    return slug


def main():
    os.makedirs(PACKS, exist_ok=True)
    slugs = []
    for kind, display, palettes, human in KINDS:
        slugs.append(build_pack(kind, display, None, human))
        for token in palettes:
            slugs.append(build_pack(kind, display, token, human))
    print(f"built {len(slugs)} packs into {PACKS}")
    for s in slugs:
        print(" ", s)


if __name__ == "__main__":
    sys.exit(main())
