#!/usr/bin/env python3
"""GO-ORBITTY-LOOK-AT — 8-dir eye cells + reaction overlay from blessed Spirit.

Steal Kamran's look-at *contract* (angle → cell, dead zone, hysteresis).
Do not redraw the body. Do not run a 360 extract. NES-honest: one idle
pose, 8 eye offsets, 6 overlay cells. Shared pixels from idle_0 / bust_0.

Outputs (bundled via tauri.conf.json directory resource):
    look/look_0.png … look_7.png          E NE N NW W SW S SE
    look/bust_0.png … bust_7.png
    look/overlay_{blink,poke,sleepy,sparkle,delighted,dizzy}.png
    look/overlay_bust_{…}.png
Review only (never bundled): look_strip.png, overlay_strip.png,
look_bust_strip.png, overlay_bust_strip.png next to this script.
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

DIR = Path(__file__).resolve().parent
OUT = DIR / "look"
TW, TH = 256, 384
# Dark cluster = the one eye. idle_0 has no other blob under this floor.
EYE_SUM = 110
# 0° = east, 45° steps. Modest offsets so the eye stays on the head bulb.
LOOK_OFFSETS = (
    (6, 0),  # E
    (5, -8),  # NE
    (0, -10),  # N
    (-14, -8),  # NW
    (-22, 0),  # W
    (-14, 8),  # SW
    (0, 10),  # S
    (5, 8),  # SE
)
OVERLAY_ORDER = (
    "blink",
    "poke",
    "sleepy",
    "sparkle",
    "delighted",
    "dizzy",
)
CREAM = (255, 246, 197, 255)
GOLD = (245, 174, 32, 255)
GOLD_HI = (255, 245, 194, 255)
LID = (214, 201, 148, 255)
PINK = (228, 139, 139, 255)


def load(path: Path) -> Image.Image:
    return Image.open(path).convert("RGBA")


def eye_pixels(im: Image.Image) -> list[tuple[int, int, tuple[int, int, int, int]]]:
    px = im.load()
    w, h = im.size
    out: list[tuple[int, int, tuple[int, int, int, int]]] = []
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a >= 200 and (r + g + b) <= EYE_SUM:
                out.append((x, y, (r, g, b, a)))
    if len(out) < 20:
        raise SystemExit(f"eye cluster too small: {len(out)}")
    return out


def opaque(px, x: int, y: int, w: int, h: int) -> bool:
    return 0 <= x < w and 0 <= y < h and px[x, y][3] >= 16


def fill_eye(px, orig_px, eye, w: int, h: int, inward: int) -> None:
    # Replace the eye with face cream sampled toward the head interior (left).
    for x, y, _ in eye:
        sx = x + inward
        if opaque(orig_px, sx, y, w, h) and (orig_px[sx, y][0] + orig_px[sx, y][1] + orig_px[sx, y][2]) > EYE_SUM:
            px[x, y] = orig_px[sx, y]
        else:
            px[x, y] = CREAM


def stamp_eye(px, orig_px, eye, dx: int, dy: int, w: int, h: int) -> None:
    for x, y, c in eye:
        nx, ny = x + dx, y + dy
        if opaque(orig_px, nx, ny, w, h):
            px[nx, ny] = c


def fit_offset(orig_px, eye, dx: int, dy: int, w: int, h: int) -> tuple[int, int]:
    # Shrink the offset until most of the eye lands on the body.
    for _ in range(12):
        hits = sum(1 for x, y, _ in eye if opaque(orig_px, x + dx, y + dy, w, h))
        if hits >= int(len(eye) * 0.9):
            return dx, dy
        dx = int(dx * 0.7)
        dy = int(dy * 0.7)
    return 0, 0


def look_cell(base: Image.Image, dx: int, dy: int, inward: int) -> Image.Image:
    im = base.copy()
    orig_px = base.load()
    px = im.load()
    w, h = im.size
    eye = eye_pixels(base)
    dx, dy = fit_offset(orig_px, eye, dx, dy, w, h)
    fill_eye(px, orig_px, eye, w, h, inward)
    stamp_eye(px, orig_px, eye, dx, dy, w, h)
    return im


def plus(px, x: int, y: int, w: int, h: int) -> None:
    for d in range(-2, 3):
        if 0 <= x + d < w and 0 <= y < h:
            px[x + d, y] = GOLD if d != 0 else GOLD_HI
        if 0 <= x < w and 0 <= y + d < h:
            px[x, y + d] = GOLD if d != 0 else GOLD_HI


def overlay_cell(base: Image.Image, kind: str, inward: int, scale: float) -> Image.Image:
    w, h = base.size
    orig_px = base.load()
    eye = eye_pixels(base)
    xs = [p[0] for p in eye]
    ys = [p[1] for p in eye]
    cx = (min(xs) + max(xs)) // 2
    cy = (min(ys) + max(ys)) // 2

    if kind == "poke":
        nw, nh = max(8, int(w * 1.16)), max(8, int(h * 0.78))
        scaled = base.resize((nw, nh), Image.NEAREST)
        out = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        out.paste(scaled, ((w - nw) // 2, (h - nh) // 2), scaled)
        return out

    im = base.copy()
    px = im.load()

    if kind == "blink":
        fill_eye(px, orig_px, eye, w, h, inward)
        x0, x1 = min(xs), max(xs)
        for x in range(x0, x1 + 1):
            if 0 <= cy < h and 0 <= x < w:
                px[x, cy] = LID
            if 0 <= cy + 1 < h and 0 <= x < w:
                px[x, cy + 1] = eye[len(eye) // 2][2]
        return im

    if kind == "sleepy":
        fill_eye(px, orig_px, eye, w, h, inward)
        y_cut = min(ys) + max(1, (max(ys) - min(ys)) // 2)
        sleepy = [(x, y, c) for x, y, c in eye if y >= y_cut]
        stamp_eye(px, orig_px, sleepy, 0, 3, w, h)
        return im

    if kind == "sparkle":
        plus(px, cx - int(18 * scale), cy - int(16 * scale), w, h)
        plus(px, cx + int(14 * scale), cy - int(12 * scale), w, h)
        plus(px, cx - int(8 * scale), cy + int(14 * scale), w, h)
        plus(px, cx + int(16 * scale), cy + int(10 * scale), w, h)
        return im

    if kind == "delighted":
        fill_eye(px, orig_px, eye, w, h, inward)
        grown = []
        seen = set()
        for x, y, c in eye:
            for nx, ny in ((x, y), (x + 1, y), (x, y - 1), (x - 1, y)):
                if (nx, ny) not in seen:
                    seen.add((nx, ny))
                    grown.append((nx, ny, c))
        stamp_eye(px, orig_px, grown, 0, -1, w, h)
        plus(px, cx - int(16 * scale), cy - int(18 * scale), w, h)
        plus(px, cx + int(14 * scale), cy - int(14 * scale), w, h)
        return im

    if kind == "dizzy":
        fill_eye(px, orig_px, eye, w, h, inward)
        stamp_eye(px, orig_px, eye, 4, 3, w, h)
        ring = (
            (cx + int(12 * scale), cy - int(10 * scale)),
            (cx + int(16 * scale), cy),
            (cx + int(10 * scale), cy + int(12 * scale)),
            (cx - int(6 * scale), cy + int(14 * scale)),
            (cx - int(16 * scale), cy + int(4 * scale)),
            (cx - int(10 * scale), cy - int(12 * scale)),
        )
        for i, (x, y) in enumerate(ring):
            color = PINK if i % 2 == 0 else GOLD
            for ox in range(2):
                for oy in range(2):
                    nx, ny = x + ox, y + oy
                    if 0 <= nx < w and 0 <= ny < h:
                        px[nx, ny] = color
        return im

    raise SystemExit(f"unknown overlay {kind}")


def strip(frames: list[Image.Image], path: Path) -> None:
    if not frames:
        return
    w, h = frames[0].size
    sheet = Image.new("RGBA", (w * len(frames), h), (0, 0, 0, 0))
    for i, fr in enumerate(frames):
        sheet.paste(fr, (i * w, 0), fr)
    sheet.save(path)


def bake_set(
    src: Path, prefix: str, overlay_prefix: str, offsets, inward: int, scale: float
) -> tuple[list[Image.Image], list[Image.Image]]:
    base = load(src)
    looks = []
    for i, (dx, dy) in enumerate(offsets):
        cell = look_cell(base, dx, dy, inward)
        dest = OUT / f"{prefix}{i}.png"
        cell.save(dest)
        looks.append(cell)
        print(f"  {dest.name} offset=({dx},{dy})")
    overlays = []
    for name in OVERLAY_ORDER:
        cell = overlay_cell(base, name, inward, scale)
        dest = OUT / f"{overlay_prefix}{name}.png"
        cell.save(dest)
        overlays.append(cell)
        print(f"  {dest.name}")
    return looks, overlays


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    print("full")
    bust_scale = 1.6
    looks, overlays = bake_set(
        DIR / "idle_0.png",
        "look_",
        "overlay_",
        LOOK_OFFSETS,
        inward=-5,
        scale=1.0,
    )
    strip(looks, DIR / "look_strip.png")
    strip(overlays, DIR / "overlay_strip.png")
    print("bust")
    bust_off = tuple((round(dx * bust_scale), round(dy * bust_scale)) for dx, dy in LOOK_OFFSETS)
    looks_b, overlays_b = bake_set(
        DIR / "bust_0.png",
        "bust_",
        "overlay_bust_",
        bust_off,
        inward=-8,
        scale=bust_scale,
    )
    strip(looks_b, DIR / "look_bust_strip.png")
    strip(overlays_b, DIR / "overlay_bust_strip.png")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
