#!/usr/bin/env python3
"""4-frame town amble via Pixelorama cels — not a mid-leg crop.

Pixelorama model (docs):
  timeline = frames × layers; a cel is one intersection.
  onion-skin = ghost of past/future frames while you draw the current cel.
  linked / copied cels keep the torso identical; only the limbs cel changes.

Workflow here:
  1. Key + nearest-scale the idle.
  2. Segment COMPLETE boots/paws (connected components), never a vertical slice.
  3. Layer 0 = locked torso (hem stays). Layer 1 = limbs.
  4. duplicate_frame × 3, then clear_cel + redraw whole limb stamps.
  5. Pass poses lift a whole foot 1px (heel off). Contact plants both.
  6. Onion-skin review GIFs: current + blue previous + red next.

Review only. Does not install into Orbitty.
"""
from __future__ import annotations

import json
import urllib.request
from collections import deque
from pathlib import Path

from PIL import Image, ImageChops

BRIDGE = "http://127.0.0.1:7373"
HERE = Path(__file__).resolve().parent
IDLES = HERE.parent / "idles"
OUT = HERE
TARGET_H = 64
PAD_X, PAD_Y = 4, 2


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
    out = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    dst = out.load()
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a < 16:
                continue
            if r > 170 and g < 50 and b < 160:  # rose / magenta key
                continue
            keep.append((x, y))
            dst[x, y] = (r, g, b, a)
    if not keep:
        raise SystemExit("no opaque sprite after chroma")
    xs = [p[0] for p in keep]
    ys = [p[1] for p in keep]
    return out.crop((min(xs), min(ys), max(xs) + 1, max(ys) + 1))


def scale_to_height(im: Image.Image, height: int) -> Image.Image:
    w = max(1, round(im.width * (height / im.height)))
    return im.resize((w, height), Image.Resampling.NEAREST)


def pad(im: Image.Image) -> Image.Image:
    canvas = Image.new("RGBA", (im.width + PAD_X * 2, im.height + PAD_Y * 2), (0, 0, 0, 0))
    canvas.paste(im, (PAD_X, PAD_Y), im)
    return canvas


def components(mask: list[tuple[int, int]]) -> list[list[tuple[int, int]]]:
    s = set(mask)
    out = []
    while s:
        start = s.pop()
        blob = [start]
        q = deque([start])
        while q:
            x, y = q.popleft()
            for nx, ny in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
                if (nx, ny) in s:
                    s.remove((nx, ny))
                    blob.append((nx, ny))
                    q.append((nx, ny))
        out.append(blob)
    out.sort(key=lambda b: min(p[0] for p in b))
    return out


def stamp_from(im: Image.Image, coords: list[tuple[int, int]]) -> tuple[Image.Image, int, int]:
    xs = [p[0] for p in coords]
    ys = [p[1] for p in coords]
    x0, y0, x1, y1 = min(xs), min(ys), max(xs) + 1, max(ys) + 1
    st = Image.new("RGBA", (x1 - x0, y1 - y0), (0, 0, 0, 0))
    src = im.load()
    dst = st.load()
    for x, y in coords:
        dst[x - x0, y - y0] = src[x, y]
    return st, x0, y0


def erase(im: Image.Image, coords: list[tuple[int, int]]) -> Image.Image:
    out = im.copy()
    px = out.load()
    for x, y in coords:
        px[x, y] = (0, 0, 0, 0)
    return out


def compose(torso: Image.Image, stamps: list[tuple[Image.Image, int, int]]) -> Image.Image:
    fr = torso.copy()
    for st, x, y in stamps:
        fr.alpha_composite(st, (x, y))
    return fr


def boy_parts(idle: Image.Image):
    """Complete left/right boots below the hem. No vertical thigh cut."""
    px = idle.load()
    w, h = idle.size
    hem = int(h * 0.80)  # tunic hem ~ row 51 on 64
    boot_px = []
    for y in range(hem, h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a < 16:
                continue
            boot_px.append((x, y))
    blobs = [b for b in components(boot_px) if len(b) >= 8]
    if len(blobs) < 2:
        raise SystemExit(f"boy: expected 2 boots, got {len(blobs)} sizes={[len(b) for b in blobs]}")
    # two largest
    blobs = sorted(blobs, key=len, reverse=True)[:2]
    blobs = sorted(blobs, key=lambda b: min(p[0] for p in b))
    left, right = blobs
    # keep the top row of each boot on the torso so the hem never opens
    def movable(blob):
        top = min(p[1] for p in blob)
        return [p for p in blob if p[1] > top]
    left_m, right_m = movable(left), movable(right)
    torso = erase(idle, left_m + right_m)
    return torso, stamp_from(idle, left_m), stamp_from(idle, right_m)


def cat_parts(idle: Image.Image):
    """Four complete paws. Body/tail stay on torso."""
    px = idle.load()
    w, h = idle.size
    paw_y = int(h * 0.82)
    paw_px = []
    for y in range(paw_y, h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if a < 16:
                continue
            paw_px.append((x, y))
    blobs = [b for b in components(paw_px) if len(b) >= 6]
    # belly often fuses the two middle paws — split the widest blob at its thinnest column
    if len(blobs) == 3:
        widest = max(blobs, key=lambda b: max(p[0] for p in b) - min(p[0] for p in b))
        xs = range(min(p[0] for p in widest) + 3, max(p[0] for p in widest) - 2)
        col = {x: 0 for x in xs}
        for x, y in widest:
            if x in col:
                col[x] += 1
        cut = min(col, key=col.get)
        blobs = [b for b in blobs if b is not widest]
        blobs.append([p for p in widest if p[0] < cut])
        blobs.append([p for p in widest if p[0] >= cut])
        blobs = [b for b in blobs if b]
    blobs = sorted(blobs, key=lambda b: min(p[0] for p in b))
    if len(blobs) < 4:
        raise SystemExit(f"cat: expected 4 paws, got {len(blobs)} sizes={[len(b) for b in blobs]}")
    blobs = blobs[:4]
    def movable(blob):
        top = min(p[1] for p in blob)
        return [p for p in blob if p[1] > top]
    moved = [movable(b) for b in blobs]
    torso = erase(idle, [p for b in moved for p in b])
    return torso, [stamp_from(idle, b) for b in moved]


def boy_cycle(torso, left, right):
    ls, lx, ly = left
    rs, rx, ry = right
    # tiny town amble: whole boot moves 1–2px. Pass = heel off (dy -1).
    return [
        compose(torso, [(ls, lx, ly), (rs, rx, ry)]),  # contact
        compose(torso, [(ls, lx + 1, ly - 1), (rs, rx, ry)]),  # pass, far steps
        compose(torso, [(ls, lx + 2, ly), (rs, rx - 1, ry)]),  # opposite contact
        compose(torso, [(ls, lx + 2, ly), (rs, rx, ry - 1)]),  # pass, near steps
    ]


def cat_cycle(torso, paws):
    # paws L→R: hind-far, hind-near, front-far, front-near
    # diagonal gait: (hind-far + front-near) then (hind-near + front-far)
    stamps = paws  # (im,x,y)*4

    def place(deltas):
        return compose(torso, [(im, x + dx, y + dy) for (im, x, y), (dx, dy) in zip(stamps, deltas)])

    return [
        place([(0, 0), (0, 0), (0, 0), (0, 0)]),
        place([(1, -1), (0, 0), (0, 0), (1, -1)]),
        place([(1, 0), (1, 0), (1, 0), (1, 0)]),  # body of step, all planted 1px on
        place([(1, 0), (2, -1), (2, -1), (1, 0)]),
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


def draw_all(pixels: list[dict], batch: int = 2500) -> None:
    for i in range(0, len(pixels), batch):
        res = cmd("draw_pixels", {"pixels": pixels[i : i + batch]})
        if not res.get("success"):
            raise SystemExit(f"draw_pixels failed: {res}")


def onion_strip(frames: list[Image.Image]) -> Image.Image:
    """Blue = previous, red = next, full color = current. Pixelorama blue-red mode."""
    n = len(frames)
    w, h = frames[0].size
    gap = 4
    sheet = Image.new("RGBA", (n * w + (n - 1) * gap, h), (20, 20, 24, 255))
    for i, fr in enumerate(frames):
        prev = frames[(i - 1) % n]
        nxt = frames[(i + 1) % n]
        ghost = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        # previous → blue
        p = prev.copy()
        pp = p.load()
        for y in range(h):
            for x in range(w):
                r, g, b, a = pp[x, y]
                if a > 16:
                    pp[x, y] = (40, 90, 220, 110)
        ghost.alpha_composite(p)
        # next → red
        q = nxt.copy()
        qp = q.load()
        for y in range(h):
            for x in range(w):
                r, g, b, a = qp[x, y]
                if a > 16:
                    qp[x, y] = (220, 50, 50, 90)
        ghost.alpha_composite(q)
        ghost.alpha_composite(fr)
        sheet.alpha_composite(ghost, (i * (w + gap), 0))
    return sheet


def make_kind(name: str) -> None:
    src = Image.open(IDLES / f"{name}.png")
    idle = pad(scale_to_height(chroma_crop(src), TARGET_H))
    if name == "boy":
        torso, left, right = boy_parts(idle)
        frames = boy_cycle(torso, left, right)
    else:
        torso, paws = cat_parts(idle)
        frames = cat_cycle(torso, paws)

    w, h = idle.size
    kind = OUT / name
    kind.mkdir(parents=True, exist_ok=True)

    created = cmd("create_canvas", {"width": w, "height": h, "name": f"bitty-{name}-amble"})
    print("create", name, created)
    if not created.get("success"):
        raise SystemExit(created)

    add_l = cmd("add_layer", {"name": "limbs"})
    print("add_layer", add_l)

    cmd("switch_cel", {"frame": 0, "layer": 0})
    draw_all(pixels_of(torso))
    cmd("switch_cel", {"frame": 0, "layer": 1})
    limb_only = frames[0].copy()
    tp = torso.load()
    lp = limb_only.load()
    for y in range(h):
        for x in range(w):
            if tp[x, y][3] > 16:
                lp[x, y] = (0, 0, 0, 0)
    draw_all(pixels_of(limb_only))

    cmd("set_fps", {"fps": 6})

    for i in range(1, 4):
        add = cmd("add_frame", {"after_frame": i - 1})
        print("add_frame", i, add)
        # torso stays: copy cel 0/0 → i/0 (Pixelorama copy_cel, not a bitmap slice)
        cp = cmd("copy_cel", {"src_frame": 0, "src_layer": 0, "dst_frame": i, "dst_layer": 0})
        print("copy torso", cp)
        sw = cmd("switch_cel", {"frame": i, "layer": 1})
        print("switch", sw)
        limb = frames[i].copy()
        lp = limb.load()
        tp = torso.load()
        for y in range(h):
            for x in range(w):
                if tp[x, y][3] > 16:
                    lp[x, y] = (0, 0, 0, 0)
        draw_all(pixels_of(limb))

    export = cmd(
        "export_animation",
        {"path": str(kind), "mode": "frames", "prefix": f"{name}_pxo"},
    )
    print("export", export)
    sheet = cmd(
        "export_animation",
        {"path": str(kind), "mode": "spritesheet", "prefix": name, "columns": 4},
    )
    print("sheet", sheet)

    for i, fr in enumerate(frames):
        fr.save(kind / f"walk_{i}.png")
    onion = onion_strip(frames)
    onion.save(kind / f"{name}-onion.png")

    gif = kind / f"{name}.gif"
    frames[0].save(
        gif, save_all=True, append_images=frames[1:], duration=160, loop=0, disposal=2
    )
    rev = []
    for fr in frames:
        bg = Image.new("RGBA", fr.size, (208, 0, 99, 255))
        bg.alpha_composite(fr)
        rev.append(bg.convert("P", palette=Image.ADAPTIVE, colors=64))
    rev_path = kind / f"{name}-review.gif"
    rev[0].save(rev_path, save_all=True, append_images=rev[1:], duration=160, loop=0)
    print("gif", gif, rev_path)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    print("health", health())
    for name in ("boy", "cat"):
        make_kind(name)
    print("done")


if __name__ == "__main__":
    main()
