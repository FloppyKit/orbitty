#!/usr/bin/env python3
"""Punch Imagine backdrops, crop, nearest-scale. Stdlib PNG only.

Imagine ignores 'transparent' and 'keep it small'. This is the exporter.

  orbitty-crush-sprite.py in.png
  orbitty-crush-sprite.py in.png -o out.png --max 128
  orbitty-crush-sprite.py in.png --dry

Punch is an edge flood of pixels near the corner/border key (black, magenta,
green, or whatever the corners agree on). Interior dark wood is kept.
"""
from __future__ import annotations

import argparse
import os
import struct
import sys
import zlib
from collections import deque

# --- PNG 8-bit RGBA (same codec as brand/bitty-crew/tools/build_packs.py) ---


def read_png(path: str):
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path}: not a PNG")
    off = 8
    width = height = bitd = ctype = inter = None
    idat = bytearray()
    while off < len(data):
        ln, typ = struct.unpack(">I4s", data[off : off + 8])
        body = data[off + 8 : off + 8 + ln]
        if typ == b"IHDR":
            width, height, bitd, ctype, _c, _f, inter = struct.unpack(
                ">IIBBBBB", body
            )
        elif typ == b"IDAT":
            idat += body
        elif typ == b"IEND":
            break
        off += 12 + ln
    if width is None:
        raise ValueError(f"{path}: no IHDR")
    if bitd != 8 or inter != 0:
        raise ValueError(f"{path}: need 8-bit non-interlaced, got bit={bitd} inter={inter}")
    raw = zlib.decompress(bytes(idat))
    # expand to RGBA
    if ctype == 6:
        ch = 4
    elif ctype == 2:
        ch = 3
    elif ctype == 4:
        ch = 2
    elif ctype == 0:
        ch = 1
    else:
        raise ValueError(f"{path}: unsupported color type {ctype} (indexed PNG: crush after convert)")
    stride_in = width * ch
    prev = bytearray(stride_in)
    rows = []
    pos = 0
    for _y in range(height):
        filt = raw[pos]
        pos += 1
        row = bytearray(raw[pos : pos + stride_in])
        pos += stride_in
        if filt == 1:
            for i in range(ch, stride_in):
                row[i] = (row[i] + row[i - ch]) & 0xFF
        elif filt == 2:
            for i in range(stride_in):
                row[i] = (row[i] + prev[i]) & 0xFF
        elif filt == 3:
            for i in range(stride_in):
                a = row[i - ch] if i >= ch else 0
                row[i] = (row[i] + ((a + prev[i]) >> 1)) & 0xFF
        elif filt == 4:
            for i in range(stride_in):
                a = row[i - ch] if i >= ch else 0
                b = prev[i]
                c = prev[i - ch] if i >= ch else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                row[i] = (row[i] + pr) & 0xFF
        elif filt != 0:
            raise ValueError(f"{path}: bad filter {filt}")
        rows.append(row)
        prev = row
    rgba = bytearray(width * height * 4)
    for y, row in enumerate(rows):
        for x in range(width):
            i = (y * width + x) * 4
            if ctype == 6:
                rgba[i : i + 4] = row[x * 4 : x * 4 + 4]
            elif ctype == 2:
                rgba[i : i + 3] = row[x * 3 : x * 3 + 3]
                rgba[i + 3] = 255
            elif ctype == 0:
                rgba[i] = rgba[i + 1] = rgba[i + 2] = row[x]
                rgba[i + 3] = 255
            elif ctype == 4:
                rgba[i] = rgba[i + 1] = rgba[i + 2] = row[x * 2]
                rgba[i + 3] = row[x * 2 + 1]
    return width, height, rgba


def write_png(path: str, width: int, height: int, px: bytes | bytearray) -> None:
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
    os.makedirs(os.path.dirname(os.path.abspath(path)) or ".", exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)


def dist2(r, g, b, kr, kg, kb) -> int:
    return (r - kr) ** 2 + (g - kg) ** 2 + (b - kb) ** 2


def corner_key(w, h, px):
    """Median-ish of the four corners + a few border samples."""
    samples = []

    def at(x, y):
        i = (y * w + x) * 4
        samples.append((px[i], px[i + 1], px[i + 2]))

    for x, y in ((0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)):
        at(x, y)
    for x in (w // 2, w // 4, 3 * w // 4):
        at(x, 0)
        at(x, h - 1)
    for y in (h // 2,):
        at(0, y)
        at(w - 1, y)
    samples.sort()
    return samples[len(samples) // 2]


def punch_edges(w, h, px, thresh: int) -> bytearray:
    out = bytearray(px)
    kr, kg, kb = corner_key(w, h, px)
    t2 = thresh * thresh
    vis = bytearray(w * h)
    q = deque()

    def is_key(x, y) -> bool:
        i = (y * w + x) * 4
        if out[i + 3] == 0:
            return True
        return dist2(out[i], out[i + 1], out[i + 2], kr, kg, kb) <= t2

    def push(x, y):
        if x < 0 or y < 0 or x >= w or y >= h:
            return
        idx = y * w + x
        if vis[idx]:
            return
        if not is_key(x, y):
            return
        vis[idx] = 1
        q.append((x, y))

    for x in range(w):
        push(x, 0)
        push(x, h - 1)
    for y in range(h):
        push(0, y)
        push(w - 1, y)
    while q:
        x, y = q.popleft()
        i = (y * w + x) * 4
        out[i + 3] = 0
        push(x - 1, y)
        push(x + 1, y)
        push(x, y - 1)
        push(x, y + 1)
    return out


def crop_alpha(w, h, px, pad: int):
    minx, miny, maxx, maxy = w, h, -1, -1
    for y in range(h):
        row = y * w * 4
        for x in range(w):
            if px[row + x * 4 + 3]:
                if x < minx:
                    minx = x
                if y < miny:
                    miny = y
                if x > maxx:
                    maxx = x
                if y > maxy:
                    maxy = y
    if maxx < 0:
        return w, h, px
    minx = max(0, minx - pad)
    miny = max(0, miny - pad)
    maxx = min(w - 1, maxx + pad)
    maxy = min(h - 1, maxy + pad)
    nw, nh = maxx - minx + 1, maxy - miny + 1
    out = bytearray(nw * nh * 4)
    for y in range(nh):
        src = ((miny + y) * w + minx) * 4
        dst = y * nw * 4
        out[dst : dst + nw * 4] = px[src : src + nw * 4]
    return nw, nh, out


def nearest_scale(w, h, px, max_side: int):
    m = max(w, h)
    if m <= max_side:
        return w, h, px
    scale = max_side / m
    nw = max(1, int(round(w * scale)))
    nh = max(1, int(round(h * scale)))
    out = bytearray(nw * nh * 4)
    for y in range(nh):
        sy = min(h - 1, int(y * h / nh))
        for x in range(nw):
            sx = min(w - 1, int(x * w / nw))
            si = (sy * w + sx) * 4
            di = (y * nw + x) * 4
            out[di : di + 4] = px[si : si + 4]
    return nw, nh, out


def unique_rgb(w, h, px) -> int:
    s = set()
    n = w * h
    for i in range(n):
        o = i * 4
        if px[o + 3] == 0:
            continue
        s.add((px[o], px[o + 1], px[o + 2]))
        if len(s) > 4000:
            return len(s)
    return len(s)


def binary_alpha(w, h, px, cut: int = 128) -> bytearray:
    out = bytearray(px)
    n = w * h
    for i in range(n):
        o = i * 4
        out[o + 3] = 255 if out[o + 3] >= cut else 0
    return out


def median_cut_palette(colors: list[tuple[int, int, int]], n: int) -> list[tuple[int, int, int]]:
    if not colors:
        return [(0, 0, 0)]
    uniq = list(set(colors))
    if len(uniq) <= n:
        return uniq
    def box_span(box):
        return max(max(c[k] for c in box) - min(c[k] for c in box) for k in range(3))

    boxes = [uniq]
    while len(boxes) < n:
        bi = max(range(len(boxes)), key=lambda i: box_span(boxes[i]))
        box = boxes[bi]
        if len(box) < 2:
            break
        channel = max(
            range(3),
            key=lambda k: max(c[k] for c in box) - min(c[k] for c in box),
        )
        box.sort(key=lambda c: c[channel])
        mid = len(box) // 2
        boxes[bi] = box[:mid]
        boxes.append(box[mid:])
    pal = []
    for box in boxes:
        r = sum(c[0] for c in box) // len(box)
        g = sum(c[1] for c in box) // len(box)
        b = sum(c[2] for c in box) // len(box)
        pal.append((r, g, b))
    return pal


def quantize_rgba(w, h, px, n_colors: int) -> bytearray:
    cols = []
    n = w * h
    for i in range(n):
        o = i * 4
        if px[o + 3]:
            cols.append((px[o], px[o + 1], px[o + 2]))
    pal = median_cut_palette(cols, max(1, n_colors))
    # nearest map
    out = bytearray(px)
    cache = {}
    for i in range(n):
        o = i * 4
        if out[o + 3] == 0:
            continue
        key = (out[o], out[o + 1], out[o + 2])
        if key not in cache:
            cache[key] = min(pal, key=lambda c: dist2(key[0], key[1], key[2], c[0], c[1], c[2]))
        r, g, b = cache[key]
        out[o], out[o + 1], out[o + 2] = r, g, b
    return out


def write_indexed_png(path: str, width: int, height: int, px) -> None:
    """8-bit indexed PNG, palette index 0 = transparent."""
    pal = [(0, 0, 0)]  # tRNS 0
    index = {(0, 0, 0, 0): 0}
    n = width * height
    ids = bytearray(n)
    for i in range(n):
        o = i * 4
        if px[o + 3] == 0:
            ids[i] = 0
            continue
        rgb = (px[o], px[o + 1], px[o + 2])
        if rgb not in index:
            if len(pal) >= 256:
                # nearest existing
                ids[i] = min(
                    range(1, len(pal)),
                    key=lambda k: dist2(rgb[0], rgb[1], rgb[2], pal[k][0], pal[k][1], pal[k][2]),
                )
                continue
            index[rgb] = len(pal)
            pal.append(rgb)
        ids[i] = index[rgb]
    trns = bytes(0 if i == 0 else 255 for i in range(len(pal)))
    plte = b"".join(bytes(c) for c in pal)

    def chunk(typ, body):
        c = struct.pack(">I", len(body)) + typ + body
        return c + struct.pack(">I", zlib.crc32(typ + body) & 0xFFFFFFFF)

    raw = bytearray()
    for y in range(height):
        raw.append(0)
        raw += ids[y * width : (y + 1) * width]
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 3, 0, 0, 0)
    data = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"PLTE", plte)
        + chunk(b"tRNS", trns)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )
    os.makedirs(os.path.dirname(os.path.abspath(path)) or ".", exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("src")
    ap.add_argument("-o", "--out", default=None)
    ap.add_argument("--max", type=int, default=128, help="longest side (0 = no scale)")
    ap.add_argument("--thresh", type=int, default=18, help="RGB distance for key match")
    ap.add_argument("--pad", type=int, default=1)
    ap.add_argument("--no-crop", action="store_true", help="keep canvas so blit fractions stay valid")
    ap.add_argument("--no-punch", action="store_true")
    ap.add_argument("--colors", type=int, default=16, help="palette size (0 = no quantize)")
    ap.add_argument("--indexed", action="store_true", help="write 8-bit indexed PNG")
    ap.add_argument("--dry", action="store_true")
    args = ap.parse_args()
    src = os.path.abspath(args.src)
    w, h, px = read_png(src)
    before = os.path.getsize(src)
    colors0 = unique_rgb(w, h, px)
    work = px
    if not args.no_punch:
        work = punch_edges(w, h, work, args.thresh)
        work = binary_alpha(w, h, work)
    cw, ch = w, h
    if not args.no_crop:
        cw, ch, work = crop_alpha(w, h, work, args.pad)
    if args.max and args.max > 0:
        ow, oh, outpx = nearest_scale(cw, ch, work, args.max)
    else:
        ow, oh, outpx = cw, ch, work
    outpx = binary_alpha(ow, oh, outpx)
    if args.colors and args.colors > 0:
        outpx = quantize_rgba(ow, oh, outpx, args.colors)
    colors1 = unique_rgb(ow, oh, outpx)
    dest = args.out or os.path.splitext(src)[0] + f".crush{ow}x{oh}.png"
    dest = os.path.abspath(dest)
    print(
        f"{os.path.basename(src)} {w}x{h} {before}B ~{colors0}c  ->  "
        f"{ow}x{oh} {colors1}c  punch={'n' if args.no_punch else 'y'} "
        f"crop={'n' if args.no_crop else 'y'} q={args.colors}"
    )
    if args.dry:
        return 0
    if args.indexed:
        write_indexed_png(dest, ow, oh, outpx)
    else:
        write_png(dest, ow, oh, outpx)
    after = os.path.getsize(dest)
    print(f"  wrote {dest}  {after}B  ({100 * after / before:.1f}% of original)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
