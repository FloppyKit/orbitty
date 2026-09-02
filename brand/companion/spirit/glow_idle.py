#!/usr/bin/env python3
"""8-frame modern-indie glow idle from blessed v4-bright.

Same body every frame. Teal core pulse + 1px bob + fringe grow on
existing wisps. Not a mid-cut. Not Imagine jiggle.
"""
from __future__ import annotations

import math
from pathlib import Path

from PIL import Image

SRC = Path(
    "/Users/dude/goose artifacts/livesprite-tauri/brand/companion/spirit/talk_v4_bright.png"
)
OUT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion/spirit")
N = 8
TW, TH = 256, 384


def teal(r, g, b) -> bool:
    return b > 90 and g > 70 and b >= r - 10 and g >= r - 20 and (g + b) > r + 40


def ochre(r, g, b) -> bool:
    return r > 160 and 70 < g < 180 and b < 80 and r > g and g > b + 10


def cream(r, g, b, a) -> bool:
    return a > 16 and r > 150 and g > 120 and b > 80 and r >= g >= b - 20 and not ochre(r, g, b)


def lift(c, k):
    return tuple(min(255, int(round(ch + (255 - ch) * k))) for ch in c)


def main() -> None:
    base = Image.open(SRC).convert("RGBA")
    assert base.size == (TW, TH), base.size
    px0 = base.load()

    teal_pts = []
    ochre_pts = []
    cream_edge = []
    for y in range(TH):
        for x in range(TW):
            r, g, b, a = px0[x, y]
            if a < 16:
                continue
            if teal(r, g, b):
                teal_pts.append((x, y))
            elif ochre(r, g, b):
                ochre_pts.append((x, y))
            elif cream(r, g, b, a):
                for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                    xx, yy = x + dx, y + dy
                    if 0 <= xx < TW and 0 <= yy < TH and px0[xx, yy][3] < 16:
                        cream_edge.append((x, y))
                        break

    frames = []
    for i in range(N):
        # ping-pong-ish sine: breathe
        phase = 2 * math.pi * i / N
        glow = 0.5 + 0.5 * math.sin(phase)  # 0..1
        bob = int(round(math.sin(phase) * 2))  # -2..2

        fr = Image.new("RGBA", (TW, TH), (0, 0, 0, 0))
        fr.alpha_composite(base, (0, bob))
        px = fr.load()

        # pulse teal core (and a 1px halo on the high frames)
        for x, y in teal_pts:
            yy = y + bob
            if not (0 <= yy < TH):
                continue
            r, g, b, a = px[x, yy]
            if a < 16:
                continue
            lr, lg, lb = lift((r, g, b), 0.15 + 0.45 * glow)
            px[x, yy] = (lr, lg, lb, a)
            if glow > 0.65:
                for dx, dy in ((-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1)):
                    xx, y2 = x + dx, yy + dy
                    if 0 <= xx < TW and 0 <= y2 < TH and px[xx, y2][3] < 16:
                        halo = int(40 + 90 * glow)
                        px[xx, y2] = (40, min(255, 180 + int(40 * glow)), min(255, 200 + halo // 2), halo)

        for x, y in ochre_pts:
            yy = y + bob
            if not (0 <= yy < TH):
                continue
            r, g, b, a = px[x, yy]
            if a < 16:
                continue
            lr, lg, lb = lift((r, g, b), 0.08 + 0.22 * glow)
            px[x, yy] = (lr, lg, lb, a)

        # grow existing wisp tips 1px outward on inhale
        if glow > 0.55:
            for x, y in cream_edge:
                yy = y + bob
                for dx, dy in ((-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0)):
                    xx, y2 = x + dx, yy + dy
                    if 0 <= xx < TW and 0 <= y2 < TH and px[xx, y2][3] < 16:
                        r, g, b, a = px[x, yy]
                        px[xx, y2] = (r, g, b, min(200, int(80 + 100 * glow)))
                        break

        dest = OUT / f"idle_{i}.png"
        fr.save(dest)
        frames.append(fr)
        print(dest.name, "glow", round(glow, 2), "bob", bob)

    sheet = Image.new("RGBA", (TW * N, TH), (0, 0, 0, 0))
    for i, fr in enumerate(frames):
        sheet.alpha_composite(fr, (i * TW, 0))
    sheet.save(OUT / "idle.png")

    still = base.copy()
    still.save(OUT / "idle_still.png")

    bg = (18, 14, 22, 255)
    gif = []
    for fr in frames:
        c = Image.new("RGBA", (TW, TH), bg)
        c.alpha_composite(fr)
        gif.append(c.convert("P", palette=Image.Palette.ADAPTIVE, colors=64))
    (OUT / "preview.gif").unlink(missing_ok=True)
    gif[0].save(
        OUT / "preview.gif",
        save_all=True,
        append_images=gif[1:],
        duration=140,
        loop=0,
        disposal=2,
    )

    review = Image.new("RGBA", (TW * N + 3 * (N - 1), TH), bg)
    for i, fr in enumerate(frames):
        review.alpha_composite(fr, (i * (TW + 3), 0))
    review.save(OUT / "review_strip.png")
    print("sheet", OUT / "idle.png", "gif", OUT / "preview.gif")


if __name__ == "__main__":
    main()
