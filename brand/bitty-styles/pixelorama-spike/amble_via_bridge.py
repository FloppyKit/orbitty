#!/usr/bin/env python3
"""Import blessed Bitty idles into Pixelorama and stamp a tiny 4-frame town amble.

Review only. Does not install into Orbitty.app.
"""
from __future__ import annotations

import json
import urllib.request
from pathlib import Path

from PIL import Image, ImageSequence

BRIDGE = "http://127.0.0.1:7373"
IDLES = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/bitty-styles/idles")
OUT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/bitty-styles/pixelorama-spike")
TARGET_H = 64  # town-readable, not Imagine-billboard


def cmd(tool: str, params: dict | None = None) -> dict:
    body = json.dumps({"tool": tool, "params": params or {}}).encode()
    req = urllib.request.Request(
        f"{BRIDGE}/command",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.loads(resp.read().decode())


def health() -> dict:
    with urllib.request.urlopen(f"{BRIDGE}/health", timeout=5) as resp:
        return json.loads(resp.read().decode())


def chroma_crop(im: Image.Image) -> Image.Image:
    im = im.convert("RGBA")
    px = im.load()
    w, h = im.size
    keep = []
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a < 16:
                continue
            # magenta / hot-pink key
            # Imagine used rose/magenta, not always #FF00FF
            if r > 170 and g < 50 and b < 160:
                continue
            keep.append((x, y))
    if not keep:
        raise SystemExit("no opaque sprite after chroma")
    xs = [p[0] for p in keep]
    ys = [p[1] for p in keep]
    box = (min(xs), min(ys), max(xs) + 1, max(ys) + 1)
    cropped = im.crop(box)
    # punch remaining key
    out = Image.new("RGBA", cropped.size, (0, 0, 0, 0))
    src = cropped.load()
    dst = out.load()
    cw, ch = cropped.size
    for y in range(ch):
        for x in range(cw):
            r, g, b, a = src[x, y]
            if a < 16:
                continue
            if r > 170 and g < 50 and b < 160:
                continue
            dst[x, y] = (r, g, b, a)
    return out


def scale_to_height(im: Image.Image, height: int) -> Image.Image:
    if im.height <= height:
        return im
    w = max(1, round(im.width * (height / im.height)))
    return im.resize((w, height), Image.Resampling.NEAREST)


def pad(im: Image.Image, extra_x: int = 4, extra_y: int = 2) -> Image.Image:
    canvas = Image.new("RGBA", (im.width + extra_x * 2, im.height + extra_y * 2), (0, 0, 0, 0))
    canvas.paste(im, (extra_x, extra_y), im)
    return canvas


def amble_frames(idle: Image.Image) -> list[Image.Image]:
    """Torso locked. Tiny heel-lift / 1-step. Legs almost parallel."""
    w, h = idle.size
    torso_cut = int(h * 0.62)
    mid = w // 2
    torso = idle.crop((0, 0, w, torso_cut))
    legs = idle.crop((0, torso_cut, w, h))
    left = legs.crop((0, 0, mid, legs.height))
    right = legs.crop((mid, 0, w, legs.height))

    def compose(lx: int, ly: int, rx: int, ry: int) -> Image.Image:
        frame = Image.new("RGBA", idle.size, (0, 0, 0, 0))
        frame.paste(torso, (0, 0), torso)
        frame.paste(left, (lx, torso_cut + ly), left)
        frame.paste(right, (mid + rx, torso_cut + ry), right)
        return frame

    # 0 contact, 1 passing (left lift), 2 opposite contact, 3 passing (right lift)
    return [
        compose(0, 0, 0, 0),
        compose(1, -1, -1, 0),
        compose(0, 0, 0, 0),
        compose(-1, 0, 1, -1),
    ]


def pixels_of(im: Image.Image) -> list[dict]:
    px = im.load()
    w, h = im.size
    out = []
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a < 16:
                continue
            out.append({"x": x, "y": y, "color": f"#{r:02x}{g:02x}{b:02x}"})
    return out


def draw_all(pixels: list[dict], batch: int = 4000) -> None:
    for i in range(0, len(pixels), batch):
        chunk = pixels[i : i + batch]
        res = cmd("draw_pixels", {"pixels": chunk})
        if not res.get("success"):
            raise SystemExit(f"draw_pixels failed: {res}")


def make_kind(name: str) -> Path:
    src = Image.open(IDLES / f"{name}.png")
    sprite = pad(scale_to_height(chroma_crop(src), TARGET_H))
    frames = amble_frames(sprite)
    w, h = sprite.size
    kind_dir = OUT / name
    kind_dir.mkdir(parents=True, exist_ok=True)

    created = cmd("create_canvas", {"width": w, "height": h, "name": f"bitty-{name}-amble"})
    print("create", name, created)
    if not created.get("success"):
        raise SystemExit(created)

    cmd("set_fps", {"fps": 6})
    for i, fr in enumerate(frames):
        if i > 0:
            add = cmd("add_frame", {"after_frame": i - 1})
            print("add_frame", i, add)
            sw = cmd("switch_frame", {"index": i})
            print("switch", sw)
        draw_all(pixels_of(fr))
        fr.save(kind_dir / f"walk_{i}.png")

    export = cmd(
        "export_animation",
        {"path": str(kind_dir), "mode": "frames", "prefix": f"{name}_pxo"},
    )
    print("export", export)
    sheet = cmd(
        "export_animation",
        {"path": str(kind_dir), "mode": "spritesheet", "prefix": name, "columns": 4},
    )
    print("sheet", sheet)

    gif_path = kind_dir / f"{name}.gif"
    frames[0].save(
        gif_path,
        save_all=True,
        append_images=frames[1:],
        duration=160,
        loop=0,
        disposal=2,
        transparency=0,
    )
    # also a magenta-backed review gif so feet read
    rev = []
    for fr in frames:
        bg = Image.new("RGBA", fr.size, (208, 0, 99, 255))
        bg.alpha_composite(fr)
        rev.append(bg.convert("P", palette=Image.ADAPTIVE, colors=64))
    rev_path = kind_dir / f"{name}-review.gif"
    rev[0].save(rev_path, save_all=True, append_images=rev[1:], duration=160, loop=0)
    print("gif", gif_path, rev_path)
    return rev_path


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    print("health", health())
    for name in ("boy", "cat"):
        make_kind(name)
    print("done", OUT)


if __name__ == "__main__":
    main()
