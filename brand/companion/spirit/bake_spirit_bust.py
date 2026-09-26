#!/usr/bin/env python3
"""GO-ORBITTY-HEADROOM-SPIRIT-ROLLBACK — spirit bust rebake (tight closeup).

Second render of the blessed v5 rest idle + v9 thinking swap: same pixels,
same 8 frames @ 140 ms, re-framed as the OLD tight closeup — the wisp face
filling the stage — on a 256×384 canvas. Hard alpha, nearest-neighbor —
a baked re-frame, NOT a runtime CSS crop.

GO-ORBITTY-COMPANION-HEADROOM's first camera scanned for a neck pinch and
kept a collar hint; a wisp has no neck, so the "bust" came out as almost
the whole figure (the timid reframe, killed 2026-08-28). The rollback
camera is the pre-Headroom closeup: the top 128px of the frame (the head
bulb with the face), tightened to the content bbox, nearest-scaled onto
the canvas.

One fixed camera per set: the crop rect is computed from frame 0 and
applied to all 8 frames, so the idle bob animates inside the bust frame
instead of the frame chasing the subject.

Outputs (ship paths, bundled via tauri.conf.json resources):
    bust_0.png … bust_7.png
    thinking/bust_0.png … thinking/bust_7.png
Review only (never bundled): bust_strip.png, thinking/bust_strip.png.
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

DIR = Path(__file__).resolve().parent
N = 8
TW, TH = 256, 384
ALPHA_FLOOR = 16
# Closeup framing: the old Head crop was the top 128px of the 256×384
# frame. The tight camera takes that region's content bbox with a small
# side/top pad (bottom stays at 128 — the faithful closeup cut), then
# fit-scales onto the canvas. Content centers at 46% of the canvas
# height (portrait bias).
CLOSEUP_TOP = 128
PAD = 6
EYE_LINE = 0.46


def alpha_bbox(im: Image.Image) -> tuple[int, int, int, int]:
    W, H = im.size
    px = im.load()
    minx, miny, maxx, maxy = W, H, -1, -1
    for y in range(H):
        for x in range(W):
            if px[x, y][3] >= ALPHA_FLOOR:
                if x < minx:
                    minx = x
                if x > maxx:
                    maxx = x
                if y < miny:
                    miny = y
                if y > maxy:
                    maxy = y
    if maxx < 0:
        raise SystemExit("empty frame — no opaque pixels")
    return minx, miny, maxx, maxy


def bust_rect(frame0: Image.Image) -> tuple[int, int, int, int]:
    # The old closeup window: top CLOSEUP_TOP rows of the frame.
    region = frame0.crop((0, 0, frame0.size[0], CLOSEUP_TOP))
    minx, miny, maxx, maxy = alpha_bbox(region)
    x0 = max(0, minx - PAD)
    y0 = max(0, miny - PAD)
    x1 = min(frame0.size[0], maxx + 1 + PAD)
    y1 = min(CLOSEUP_TOP, maxy + 1)  # bottom cut stays at the closeup edge
    return x0, y0, x1, y1


def reframe(src: Image.Image, rect: tuple[int, int, int, int]) -> Image.Image:
    x0, y0, x1, y1 = rect
    crop = src.crop((x0, y0, x1, y1))
    w, h = crop.size
    s = min(TW / w, TH / h)
    nw, nh = max(1, round(w * s)), max(1, round(h * s))
    big = crop.resize((nw, nh), Image.Resampling.NEAREST)
    out = Image.new("RGBA", (TW, TH), (0, 0, 0, 0))
    ox = (TW - nw) // 2
    oy = round(TH * EYE_LINE - nh / 2)
    out.alpha_composite(big, (ox, oy))
    return out


def bake_set(src_dir: Path, dst_dir: Path, src_name: str, dst_name: str) -> list[Path]:
    frames = []
    for i in range(N):
        p = src_dir / src_name.format(i)
        if not p.exists():
            raise SystemExit(f"missing source frame {p}")
        frames.append(Image.open(p).convert("RGBA"))
    rect = bust_rect(frames[0])
    print(f"{src_dir}/{src_name}: rect={rect} {rect[2]-rect[0]}x{rect[3]-rect[1]}")
    out_paths = []
    for i, fr in enumerate(frames):
        bust = reframe(fr, rect)
        dst = dst_dir / dst_name.format(i)
        dst_dir.mkdir(parents=True, exist_ok=True)
        bust.save(dst)
        out_paths.append(dst)
        print(" ", dst.name)
    return out_paths


def write_strip(paths: list[Path], out: Path) -> None:
    bg = (18, 14, 22, 255)
    strip = Image.new("RGBA", (TW * len(paths) + 3 * (len(paths) - 1), TH), bg)
    for i, p in enumerate(paths):
        strip.alpha_composite(Image.open(p).convert("RGBA"), (i * (TW + 3), 0))
    strip.save(out)
    print("strip", out)


def main() -> None:
    rest = bake_set(DIR, DIR, "idle_{}.png", "bust_{}.png")
    think = bake_set(DIR / "thinking", DIR / "thinking", "think_{}.png", "bust_{}.png")
    write_strip(rest, DIR / "bust_strip.png")
    write_strip(think, DIR / "thinking" / "bust_strip.png")


if __name__ == "__main__":
    main()
