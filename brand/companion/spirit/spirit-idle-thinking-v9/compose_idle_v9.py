#!/usr/bin/env python3
"""Orbitty companion glow idle v9 — art compositor.

v8 direction kept: punchy v5 rose at rest, space-dust violet at
peak, live two-pole mixer, 1px hard dust rim. v9 is a modest
even-out + a little more felt purple — not a new character.

v8 miss: the 1px bloom and the traveling mix-wave were gated to
the upper silhouette (Y_EDGE / yy<210). Purple ate the rose at
the head and barely touched the tail. v9 runs the space-dust mix
AND the 1px bloom the full height of the silhouette.

Poles (unchanged, already sampled):
  rose        #CC4963  (204, 73, 99)   — v5 maroon mean, KEEP
  space-dust  #7C3CA5  (124, 60, 165)  — glow-ring mean (body)
  bloom       #A654D1  (166, 84, 209)  — bright glow (peak outer)
  high-sat    #9750B9  (151, 80, 185)  — high-sat ring
  bloom+      #AA56D6  (170, 86, 214)  — 1px only, hair brighter

Do NOT use icon dusty lavender #967691 (v7 miss, too subtle).
Do NOT use v6 grape #8B5478. Do not go neon candy.

Each rose pixel keeps its own chromaticity and luma / fold shading.
Time: striking rose at rest → space-dust violet at peak → rose.
Space: inner crease / shadowed folds stay rose; outer rim + the
traveling edge-wave take the space-dust purple at peak — now
head, sides, AND lower wisps/tail. No upper-half gate on color.

At pulse peak a hard 1px brighter-violet dust rim sits on the
outer edge, full silhouette (ship PNGs stay hard-alpha 0/255).
No extra specks. Do not fatten the ring into a sticker.

Cream, teal, ochre, and the eye disk are never remapped.
Outline still fades toward cream at rest, fuller at glow peak.
GIF palette reserves the pupil darks.

Motion locks unchanged: edge-wave stays upper-rim (Y_EDGE is
motion only). Do not overwrite v5/v6/v7/v8.
"""
from __future__ import annotations

from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path("/workspace/orbitty-spirit")
BLESSED = ROOT / "talk_v4_bright.png"
GLOW_DIR = ROOT / "out"  # untranslated v1 core pulse
OUT = ROOT / "out_v9"

W, H = 256, 384
N = 8
DURATION_MS = 140

CREAM = np.array([255, 245, 194], dtype=np.float32)
OUTLINE_MIX_MAX = 0.44

# Two-pole halo. Mix uses each pixel's own rose chromaticity — these
# hexes are the named poles for reports / compare swatches.
# Rose pole: v5 rest/peak maroon mean ~#CC4963 (204, 73, 99). KEEP.
# Space-dust: Orbitty.jpg glow-ring mean #7C3CA5, bloom #A654D1.
# Cooler, more electric, more B than R. Nebula, not grape plum.
ROSE_POLE = np.array([204.0, 73.0, 99.0], dtype=np.float32)  # #CC4963
SPACE_DUST = np.array([124.0, 60.0, 165.0], dtype=np.float32)  # #7C3CA5
SPACE_BLOOM = np.array([166.0, 84.0, 209.0], dtype=np.float32)  # #A654D1
SPACE_HISAT = np.array([151.0, 80.0, 185.0], dtype=np.float32)  # #9750B9
# 1px bloom only: #A654D1 luma-lifted ~3% (more luma, keep B>R).
# Not a new pole — just a hair brighter so the rim is felt.
SPACE_BLOOM_BRIGHT = np.array([170.0, 86.0, 214.0], dtype=np.float32)  # #AA56D6

# Time: rest stays striking rose; peak envelope is high enough that
# the OUTER rim actually lands on space-dust violet (v7 HALO_PEAK
# 0.94 + timid #967691 left peak outer at ~#9C637E rose-mauve).
HALO_REST = 0.05
HALO_PEAK = 1.00
# Inner crease keeps this fraction of the time mix (stays more rose).
# A little live whisper so folds still ride the cycle (Matt liked that).
HALO_SPACE_FLOOR = 0.08
# Lift luma toward the bloom's native luma (~110) as mix rises so
# dark outer pixels glow instead of collapsing to muddy grape.
HALO_LUMA_LIFT = 0.55
# Extra RGB pull on the TRUE outer 1px toward brighter bloom at peak.
# v8 was 0.58; +14% so peak outer lands a notch closer to #A654D1
# / #AA56D6. Folds still gated — do not raise crease mix.
HALO_OUTER_PULL = 0.66

# Spatial lock / travel
Y_HEAD = 78.0
Y_TAIL = 328.0
CORE_Y, CORE_X = 188.0, 134.0
CORE_R0, CORE_R1 = 28.0, 72.0

# Eye cluster — never shear these pixels; stamp after compose
EYE_Y, EYE_X, EYE_R = 53.4, 197.9, 11.0
EYE_DARK_MAX = 80  # dark brown/black pupil only

# Upper-half edge wave MOTION only (lock). Color mix does NOT use this.
Y_EDGE_FULL = 118.0
Y_EDGE_FADE = 178.0
EDGE_BAND_IN = 1.4   # full weight on/near the rim
EDGE_BAND_OUT = 4.0  # zero by ~4 px inside
EDGE_TRAVEL_CAP = 2.0


def pulse_amount(i: int) -> float:
    """0 at frame 0, 1 at frame 4. Matches v1/v2 teal core envelope."""
    return 0.5 * (1.0 - np.cos(2.0 * np.pi * i / N))


def ease_wave(i: int) -> float:
    """Looping ease-in-out phase angle, 8 frames."""
    return 2.0 * np.pi * i / N


def smoothstep(a: float, b: float, v: np.ndarray) -> np.ndarray:
    t = np.clip((v - a) / (b - a), 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def load_rgba(path: Path) -> np.ndarray:
    return np.array(Image.open(path).convert("RGBA"), dtype=np.uint8)


def _n8(flag: np.ndarray) -> np.ndarray:
    n = np.zeros_like(flag)
    for dy in (-1, 0, 1):
        for dx in (-1, 0, 1):
            if dy == 0 and dx == 0:
                continue
            rolled = np.roll(np.roll(flag, dy, 0), dx, 1)
            # prevent wrap-around ghosts at canvas edges
            if dy == -1:
                rolled[-1, :] = False
            elif dy == 1:
                rolled[0, :] = False
            if dx == -1:
                rolled[:, -1] = False
            elif dx == 1:
                rolled[:, 0] = False
            n |= rolled
    return n


def eye_disk_mask() -> np.ndarray:
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    return np.hypot(xx - EYE_X, yy - EYE_Y) <= EYE_R


def eye_pupil_mask(src: np.ndarray) -> np.ndarray:
    """Dark brown/black eye pixels inside the eye disk."""
    disk = eye_disk_mask()
    rgb = src[:, :, :3].astype(np.int16)
    a = src[:, :, 3] > 0
    dark = a & (rgb.max(axis=2) < EYE_DARK_MAX)
    return disk & dark


def cream_tick_mask(glow: np.ndarray) -> np.ndarray:
    """1px white/cream gleam only — never red or teal."""
    disk = eye_disk_mask()
    rgb = glow[:, :, :3].astype(np.int16)
    a = glow[:, :, 3] > 0
    r, g, b = rgb[:, :, 0], rgb[:, :, 1], rgb[:, :, 2]
    cream = (r >= 240) & (g >= 230) & (b >= 180) & (r - g <= 20) & (g > b)
    return disk & a & cream


def outline_mask(src: np.ndarray) -> np.ndarray:
    rgb = src[:, :, :3].astype(np.int16)
    a = src[:, :, 3] > 0
    r, g, b = rgb[:, :, 0], rgb[:, :, 1], rgb[:, :, 2]
    maroon = a & (r > 170) & (g < 110) & (b > 60) & (b < 150) & (r > g + 70)
    trans = ~a
    teal = a & (g > r + 10) & (b > r) & (g > 80)
    m = maroon & (_n8(trans) | _n8(teal))
    # body rim only — never fade the eye disk
    return m & ~eye_disk_mask()


def outer_rim_mask(src: np.ndarray) -> np.ndarray:
    """Maroon silhouette only (touches transparent). Not the teal-cavity rim."""
    rgb = src[:, :, :3].astype(np.int16)
    a = src[:, :, 3] > 0
    r, g, b = rgb[:, :, 0], rgb[:, :, 1], rgb[:, :, 2]
    maroon = a & (r > 170) & (g < 110) & (b > 60) & (b < 150) & (r > g + 70)
    return maroon & _n8(~a)


def rose_halo_mask(src: np.ndarray) -> np.ndarray:
    """Original rose/maroon rim — same family as v5.

    Detector: r>170, g<110, 60<b<150, r>g+70. Excludes cream, teal,
    ochre (low blue), and the eye disk. Do not remap those.
    """
    rgb = src[:, :, :3].astype(np.int16)
    a = src[:, :, 3] > 0
    r, g, b = rgb[:, :, 0], rgb[:, :, 1], rgb[:, :, 2]
    rose = a & (r > 170) & (g < 110) & (b > 60) & (b < 150) & (r > g + 70)
    return rose & ~eye_disk_mask()


def _luma(rgb: np.ndarray) -> np.ndarray:
    return (
        0.2126 * rgb[..., 0] + 0.7152 * rgb[..., 1] + 0.0722 * rgb[..., 2]
    )


def halo_mix_field(
    i: int,
    outer_space: np.ndarray,
    lit_space: np.ndarray,
    edge_w: np.ndarray,
) -> np.ndarray:
    """Per-pixel mix: 0 = original rose chrom, 1 = space-dust violet.

    Time follows pulse_amount / ease_wave so the 8 frames breathe
    rose → space-dust → rose with the teal core (not a strobe).
    Space: inner crease / shadowed fold stays more rose; outer lit
    rim and the traveling edge-wave catch more space-dust purple.
    Outer weight is higher than v7 so peak OUTER actually lands.
    """
    p = float(pulse_amount(i))
    time_amt = HALO_REST + (HALO_PEAK - HALO_REST) * p

    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    th = ease_wave(i)
    # same column-phase family as edge_displacement_fields
    band = np.floor(xx / 5.0)
    phase = th + band * 0.82 + 0.35
    wave_catch = (0.5 * (1.0 + np.sin(phase))) * edge_w

    # v9: outer rim is the even-out term (full silhouette, no Y gate).
    # 0.80 → 0.92 so tail outer mix is ~+13% vs v8's 0.816 when the
    # traveling wave is absent. Wave is now full-height accent only.
    space = HALO_SPACE_FLOOR + (1.0 - HALO_SPACE_FLOOR) * np.clip(
        0.92 * outer_space + 0.10 * lit_space + 0.22 * wave_catch,
        0.0,
        1.0,
    )
    return (time_amt * space).astype(np.float32)


def halo_target_field(
    outer_space: np.ndarray,
    lit_space: np.ndarray,
) -> np.ndarray:
    """Per-pixel purple pole: dust body, bloom on outer/lit rim.

    #7C3CA5 in the folds; peak outer/lit blooms toward #A654D1
    with a touch of #9750B9 so it stays nebula, not one flat chip.
    """
    bloom_w = np.clip(0.78 * outer_space + 0.22 * lit_space, 0.0, 1.0)
    # mid-outer leans hisat, true outer leans bloom
    mid = (SPACE_DUST * 0.35 + SPACE_HISAT * 0.65).astype(np.float32)
    # 0..low bloom_w: dust → hisat; high: hisat → bloom
    t = bloom_w
    lo = SPACE_DUST[None, None, :] * (1.0 - t)[:, :, None] + mid[None, None, :] * t[:, :, None]
    hi = mid[None, None, :] * (1.0 - t)[:, :, None] + SPACE_BLOOM[None, None, :] * t[:, :, None]
    # blend lo/hi by t itself so outer is bloom
    tgt = lo * (1.0 - t)[:, :, None] + hi * t[:, :, None]
    return tgt.astype(np.float32)


def mix_halo_live(
    src: np.ndarray,
    rose_mask: np.ndarray,
    mix_amt: np.ndarray,
    target_rgb: np.ndarray,
    outer_space: np.ndarray,
) -> np.ndarray:
    """Chromaticity-mix rose pixels toward space-dust. Keep fold luma.

    Never stamps one purple on every rim pixel. High-luma rose-cream
    AA is only nudged so it cannot go magenta. Outer rim gets an
    extra RGB pull toward #A654D1 so peak outer actually reads violet.
    """
    out = src.copy()
    if not rose_mask.any():
        return out
    pix = out[:, :, :3][rose_mask].astype(np.float32)
    amt = mix_amt[rose_mask]
    tgt = target_rgb[rose_mask]
    outer = outer_space[rose_mask]
    y = _luma(pix)
    # high-luma AA: collapse mix so we only nudge hue
    hi = smoothstep(145.0, 180.0, y)
    amt = amt * (1.0 - 0.78 * hi)

    s = np.maximum(pix.sum(axis=1), 1.0)
    chrom = pix / s[:, None]
    tsum = np.maximum(tgt.sum(axis=1), 1.0)
    tchrom = tgt / tsum[:, None]
    new_chrom = chrom * (1.0 - amt)[:, None] + tchrom * amt[:, None]
    new_pix = new_chrom * s[:, None]
    ny = _luma(new_pix)
    new_pix = new_pix * (y / np.maximum(ny, 1e-6))[:, None]

    y_purple = _luma(tgt)
    y_out = y + amt * np.maximum(0.0, y_purple - y) * HALO_LUMA_LIFT
    new_pix = new_pix * (y_out / np.maximum(_luma(new_pix), 1e-6))[:, None]

    # true outer 1px only — gate so inner creases stay rose.
    rim_gate = smoothstep(0.62, 0.92, np.clip(outer, 0.0, 1.0))
    pull = np.clip(amt * rim_gate * HALO_OUTER_PULL, 0.0, 1.0)
    # do not flatten high-luma AA
    pull = pull * (1.0 - 0.85 * hi)
    new_pix = new_pix * (1.0 - pull)[:, None] + SPACE_BLOOM_BRIGHT[None, :] * pull[:, None]
    # restore a bit of luma after the pull so folds next to the rim
    # do not all collapse to one chip — only the pulled pixels shift
    y2 = _luma(new_pix)
    keep = 0.35  # keep some of the post-pull luma (glow), 65% original structure
    y_mix = y * (1.0 - keep * pull) + y2 * (keep * pull)
    new_pix = new_pix * (y_mix / np.maximum(y2, 1e-6))[:, None]

    out[:, :, :3][rose_mask] = np.clip(np.rint(new_pix), 0, 255).astype(np.uint8)
    return out


def add_hard_dust_rim(arr: np.ndarray, pulse: float) -> np.ndarray:
    """Hard 1px space-dust rim on the outer edge at pulse peak.

    Ship frames stay hard-alpha. No extra specks — only a 1px ring
    of already-silhouette-adjacent pixels, tinted bloom/dust.
    Existing outer halo pixels are punched toward #AA56D6 so the
    rim reads as luminous nebula dust. A new 1px ring outside the
    silhouette appears only near peak (smooth envelope, not a strobe),
    now the FULL silhouette — head, sides, and lower wisps/tail.
    Still 1px; do not fatten into a sticker. Eye disk, cream, teal,
    ochre are never painted.
    """
    out = arr.copy()
    p = float(pulse)
    if p <= 0.04:
        return out

    a = out[:, :, 3] > 0
    disk = eye_disk_mask()
    # small keep-off around the eye so the face disk stays clean
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    eye_keep = np.hypot(xx - EYE_X, yy - EYE_Y) <= (EYE_R + 2.5)

    rgb = out[:, :, :3].astype(np.int16)
    r, g, b = rgb[:, :, 0], rgb[:, :, 1], rgb[:, :, 2]
    cream = (r >= 240) & (g >= 230) & (b >= 180) & (r - g <= 20) & (g > b)
    teal = (g > r + 10) & (b > r) & (g > 80)
    ochre = (r > 180) & (g > 120) & (g < 210) & (b < 80)
    # halo family: original rose OR already-mixed space-dust
    rose = (r > 170) & (g < 110) & (b > 60) & (b < 160) & (r > g + 70)
    violet = (b > 90) & (r > 70) & (g < 150) & (b + 8 > r) & (b > g + 10)
    halo = a & ~disk & ~cream & ~teal & ~ochre & (rose | violet)

    # punch existing outer halo toward bloom (scales with pulse).
    # This IS the hard 1px brighter-violet rim — we do not grow the
    # silhouette (a +2k px ring on 3 frames read as a size strobe).
    outer_exist = halo & _n8(~a)
    if outer_exist.any():
        pix = out[:, :, :3][outer_exist].astype(np.float32)
        y = _luma(pix)
        amt = 0.48 * p  # v8 was 0.42; modest +felt, still folded
        pix = pix * (1.0 - amt) + SPACE_BLOOM_BRIGHT[None, :] * amt
        # restore most original luma so the rim stays folded, not flat
        pix = pix * (y / np.maximum(_luma(pix), 1e-6))[:, None]
        # small glow lift toward brighter-bloom luma
        yb = float(_luma(SPACE_BLOOM_BRIGHT))
        y2 = y + amt * np.maximum(0.0, yb - y) * 0.46
        pix = pix * (y2 / np.maximum(_luma(pix), 1e-6))[:, None]
        out[:, :, :3][outer_exist] = np.clip(np.rint(pix), 0, 255).astype(np.uint8)

    # optional 1px dust JUST outside, on the already-violet rim at peak.
    # v9: NO upper-half / Y_EDGE gate — tail and lower wisps get the
    # same 1px bloom the head does. Still 1px (not a sticker ring).
    grow = smoothstep(0.78, 0.96, np.array([p], dtype=np.float32))[0]
    if grow > 0.15:
        rgb2 = out[:, :, :3].astype(np.int16)
        r2, g2, b2 = rgb2[:, :, 0], rgb2[:, :, 1], rgb2[:, :, 2]
        already_v = outer_exist & (b2 > r2 + 18) & (b2 > 140)
        new_outer = _n8(already_v) & ~a & ~eye_keep & ~disk
        if new_outer.any():
            # lean harder on brighter bloom; hisat only a whisper
            col = SPACE_BLOOM_BRIGHT * (0.82 + 0.18 * grow) + SPACE_HISAT * (0.18 * (1.0 - grow))
            col = np.clip(np.rint(col), 0, 255).astype(np.uint8)
            out[new_outer, 0] = col[0]
            out[new_outer, 1] = col[1]
            out[new_outer, 2] = col[2]
            out[new_outer, 3] = 255
    return out


def fade_outline(src: np.ndarray, mix: float) -> np.ndarray:
    out = src.copy()
    if mix <= 0:
        return out
    mask = outline_mask(src)
    pix = out[:, :, :3].astype(np.float32)
    pix[mask] = pix[mask] * (1.0 - mix) + CREAM * mix
    out[:, :, :3] = np.clip(np.rint(pix), 0, 255).astype(np.uint8)
    return out


def overlay_glow(body: np.ndarray, glow: np.ndarray) -> np.ndarray:
    out = body.copy()
    gm = glow[:, :, 3] > 0
    out[gm] = glow[gm]
    return out


def harden_alpha(src: np.ndarray) -> np.ndarray:
    out = src.copy()
    vis = out[:, :, 3] > 0
    out[:, :, 3] = np.where(vis, 255, 0).astype(np.uint8)
    out[~vis] = 0
    return out


def components(alpha: np.ndarray) -> list[np.ndarray]:
    H, W = alpha.shape
    visited = np.zeros((H, W), dtype=bool)
    comps: list[np.ndarray] = []
    ys, xs = np.where(alpha)
    for y, x in zip(ys.tolist(), xs.tolist()):
        if visited[y, x]:
            continue
        stack = [(y, x)]
        visited[y, x] = True
        pts = []
        while stack:
            cy, cx = stack.pop()
            pts.append((cy, cx))
            for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                ny, nx = cy + dy, cx + dx
                if 0 <= ny < H and 0 <= nx < W and alpha[ny, nx] and not visited[ny, nx]:
                    visited[ny, nx] = True
                    stack.append((ny, nx))
        comps.append(np.array(pts, dtype=np.int32))
    comps.sort(key=lambda p: len(p), reverse=True)
    return comps


def extract_specks(src: np.ndarray) -> tuple[np.ndarray, list[np.ndarray]]:
    """Return body without the two blessed specks, plus speck sprites (tight)."""
    a = src[:, :, 3] > 0
    comps = components(a)
    # two mid-size islands (not the main body, not 1px dust)
    specks = [c for c in comps[1:] if 20 <= len(c) <= 800]
    specks = specks[:2]
    body = src.copy()
    stamps = []
    for pts in specks:
        ys, xs = pts[:, 0], pts[:, 1]
        y0, y1 = int(ys.min()), int(ys.max()) + 1
        x0, x1 = int(xs.min()), int(xs.max()) + 1
        stamp = np.zeros((y1 - y0, x1 - x0, 4), dtype=np.uint8)
        stamp[ys - y0, xs - x0] = src[ys, xs]
        stamps.append((stamp, y0, x0))
        body[ys, xs] = 0
    return body, stamps


def paste_sprite(dst: np.ndarray, stamp: np.ndarray, y0: int, x0: int) -> None:
    sh, sw = stamp.shape[:2]
    y1, x1 = y0 + sh, x0 + sw
    sy0 = 0 if y0 >= 0 else -y0
    sx0 = 0 if x0 >= 0 else -x0
    dy0 = max(0, y0)
    dx0 = max(0, x0)
    dy1 = min(H, y1)
    dx1 = min(W, x1)
    if dy0 >= dy1 or dx0 >= dx1:
        return
    sy1 = sy0 + (dy1 - dy0)
    sx1 = sx0 + (dx1 - dx0)
    chunk = stamp[sy0:sy1, sx0:sx1]
    vis = chunk[:, :, 3] > 0
    region = dst[dy0:dy1, dx0:dx1]
    region[vis] = chunk[vis]


def chamfer_distance(seed: np.ndarray, cap: float = 8.0) -> np.ndarray:
    """Two-pass 8-neighbor chamfer distance to True pixels. No scipy."""
    inf = cap + 4.0
    d = np.full(seed.shape, inf, dtype=np.float32)
    d[seed] = 0.0
    h, w = seed.shape
    d1, d2 = 1.0, 1.41421356
    # forward
    for y in range(h):
        row = d[y]
        if y > 0:
            prev = d[y - 1]
            row[0] = min(row[0], prev[0] + d1, prev[1] + d2 if w > 1 else inf)
            for x in range(1, w - 1):
                row[x] = min(
                    row[x],
                    prev[x] + d1,
                    prev[x - 1] + d2,
                    prev[x + 1] + d2,
                    row[x - 1] + d1,
                )
            if w > 1:
                row[w - 1] = min(
                    row[w - 1],
                    prev[w - 1] + d1,
                    prev[w - 2] + d2,
                    row[w - 2] + d1,
                )
        else:
            for x in range(1, w):
                row[x] = min(row[x], row[x - 1] + d1)
    # backward
    for y in range(h - 1, -1, -1):
        row = d[y]
        if y < h - 1:
            nxt = d[y + 1]
            row[w - 1] = min(
                row[w - 1],
                nxt[w - 1] + d1,
                nxt[w - 2] + d2 if w > 1 else inf,
            )
            for x in range(w - 2, 0, -1):
                row[x] = min(
                    row[x],
                    nxt[x] + d1,
                    nxt[x - 1] + d2,
                    nxt[x + 1] + d2,
                    row[x + 1] + d1,
                )
            row[0] = min(
                row[0],
                nxt[0] + d1,
                nxt[1] + d2 if w > 1 else inf,
                row[1] + d1 if w > 1 else inf,
            )
        else:
            for x in range(w - 2, -1, -1):
                row[x] = min(row[x], row[x + 1] + d1)
    return np.minimum(d, cap + 1.0)


def motion_weight() -> np.ndarray:
    """0 at head/core, 1 at tail and side wisps. Not a global translate."""
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    w_y = smoothstep(Y_HEAD, Y_TAIL, yy)

    # extra travel on lower wisps and lateral breath-mass
    w_tail = smoothstep(250.0, 340.0, yy)
    w_left = smoothstep(95.0, 28.0, xx) * smoothstep(110.0, 165.0, yy) * (
        1.0 - smoothstep(240.0, 275.0, yy)
    )
    w_right = smoothstep(165.0, 232.0, xx) * smoothstep(145.0, 205.0, yy) * (
        1.0 - smoothstep(250.0, 285.0, yy)
    )

    # faint top-left head hairs only (not the head mass / eye)
    w_hair = (
        (1.0 - smoothstep(8.0, 52.0, yy))
        * smoothstep(200.0, 150.0, xx)
        * smoothstep(110.0, 145.0, xx)
        * 0.28
    )

    dist_core = np.hypot(xx - CORE_X, yy - CORE_Y)
    core_allow = smoothstep(CORE_R0, CORE_R1, dist_core)  # 0 inside core

    # lock head / eye cluster (top-right)
    head_lock = smoothstep(96.0, 44.0, yy) * smoothstep(150.0, 205.0, xx)
    head_allow = 1.0 - head_lock

    w = (0.72 * w_y + 0.38 * w_tail + 0.55 * w_left + 0.42 * w_right) * core_allow * head_allow
    w = np.clip(w + w_hair, 0.0, 1.0)
    return w.astype(np.float32)


def edge_band_weight(src: np.ndarray, body_w: np.ndarray) -> np.ndarray:
    """1–2 px wave support: outer maroon rim, upper half, not eye/core/interior.

    Distance-from-outline falloff: on the rim ≈ 1, dead by ~4 px inside
    the cream. Teal-cavity maroon is excluded so the diamond stays put.
    Already-waving tail/side mass is faded out so we do not stack travel.
    """
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    rim = outer_rim_mask(src)
    dist = chamfer_distance(rim, cap=8.0)

    band = 1.0 - smoothstep(EDGE_BAND_IN, EDGE_BAND_OUT, dist)
    upper = 1.0 - smoothstep(Y_EDGE_FULL, Y_EDGE_FADE, yy)

    # keep identity interiors locked
    dist_core = np.hypot(xx - CORE_X, yy - CORE_Y)
    core_lock = 1.0 - smoothstep(CORE_R0, CORE_R1, dist_core)
    eye_lock = 1.0 - smoothstep(EYE_R - 3.0, EYE_R + 1.0, np.hypot(xx - EYE_X, yy - EYE_Y))

    # do not add on top of the v3 tail/side 2–4 px wave
    already = np.clip(body_w * 1.6, 0.0, 1.0)

    w = band * upper * (1.0 - core_lock) * (1.0 - eye_lock) * (1.0 - already)
    return np.clip(w, 0.0, 1.0).astype(np.float32)


def mix_edge_band_weight(src: np.ndarray) -> np.ndarray:
    """Full-height 1–2 px rim support for the spatial purple mix.

    Same chamfer falloff as the motion edge band, but NO Y_EDGE /
    upper-half gate and no body-wave fade. Lower wisps and tail
    catch the traveling mix the way the head does. Eye / teal core
    stay locked so we do not wash the face or diamond.
    """
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    rim = outer_rim_mask(src)
    dist = chamfer_distance(rim, cap=8.0)
    band = 1.0 - smoothstep(EDGE_BAND_IN, EDGE_BAND_OUT, dist)

    dist_core = np.hypot(xx - CORE_X, yy - CORE_Y)
    core_lock = 1.0 - smoothstep(CORE_R0, CORE_R1, dist_core)
    eye_lock = 1.0 - smoothstep(EYE_R - 3.0, EYE_R + 1.0, np.hypot(xx - EYE_X, yy - EYE_Y))

    w = band * (1.0 - core_lock) * (1.0 - eye_lock)
    return np.clip(w, 0.0, 1.0).astype(np.float32)


def displacement_fields(i: int, weight: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Phase-offset column shear. Evaluated at dest coords for backward sample."""
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    th = ease_wave(i)

    # 6 px columns, ~58 px wavelength — cloth lobes, not a 90s shuffle
    band = np.floor(xx / 6.0)
    phase = th + band * 0.65

    # slower second set for side-wisp independent lean
    band2 = np.floor(xx / 11.0)
    phase2 = th + band2 * 0.90 + 1.2

    # tail leads mid-body slightly (flame/cloth, not a genie twist)
    phase_y = phase + (yy - 220.0) * 0.012

    dx = weight * (1.5 * np.sin(phase) + 0.55 * np.sin(phase2))
    dy = weight * (2.6 * np.sin(phase_y + 0.95) + 0.45 * np.sin(th + xx * 0.035))

    mag = np.hypot(dx, dy)
    over = mag > 4.0
    if np.any(over):
        s = np.where(over, 4.0 / mag, 1.0)
        dx = dx * s
        dy = dy * s
    return dx.astype(np.float32), dy.astype(np.float32)


def edge_displacement_fields(i: int, weight: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Lighter, slightly phase-offset shear for the upper maroon rim only."""
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    th = ease_wave(i)

    # narrower columns so the face rim reads as a local flutter, not a block shift
    band = np.floor(xx / 5.0)
    phase = th + band * 0.82 + 0.35
    band2 = np.floor(xx / 9.0)
    phase2 = th * 0.85 + band2 * 1.05 + 2.1
    phase_y = phase + (yy - 70.0) * 0.018

    # amplitudes stay under 2 px even at weight 1
    dx = weight * (0.85 * np.sin(phase) + 0.28 * np.sin(phase2))
    dy = weight * (1.05 * np.sin(phase_y + 0.55) + 0.22 * np.sin(th + xx * 0.05 + 1.4))

    mag = np.hypot(dx, dy)
    over = mag > EDGE_TRAVEL_CAP
    if np.any(over):
        s = np.where(over, EDGE_TRAVEL_CAP / mag, 1.0)
        dx = dx * s
        dy = dy * s
    return dx.astype(np.float32), dy.astype(np.float32)


def warp_backward(src: np.ndarray, dx: np.ndarray, dy: np.ndarray) -> np.ndarray:
    """Nearest-neighbor backward shear. Solid interior, rippled silhouette."""
    yy, xx = np.mgrid[0:H, 0:W]
    sx = np.rint(xx - dx).astype(np.int32)
    sy = np.rint(yy - dy).astype(np.int32)
    ok = (sx >= 0) & (sx < W) & (sy >= 0) & (sy < H)
    out = np.zeros_like(src)
    out[ok] = src[sy[ok], sx[ok]]
    return out


def speck_drift(i: int, k: int) -> tuple[int, int]:
    """Small independent wave; not locked to body columns."""
    th = ease_wave(i)
    if k == 0:
        dx = 1.3 * np.sin(th + 0.55)
        dy = 1.7 * np.sin(th + 2.15)
    else:
        dx = 1.1 * np.sin(th + 2.75)
        dy = 1.9 * np.sin(th + 0.70)
    return int(round(dx)), int(round(dy))


def lock_eye(idle: np.ndarray, blessed: np.ndarray, pupil: np.ndarray) -> np.ndarray:
    """Copy blessed eye pixels exactly after all warps / mix / glow."""
    out = idle.copy()
    out[pupil] = blessed[pupil]
    return out


def to_img(arr: np.ndarray) -> Image.Image:
    return Image.fromarray(harden_alpha(arr), "RGBA")


def save_gif(
    frames: list[Image.Image],
    path: Path,
    scale: int = 1,
    pupil_src: np.ndarray | None = None,
) -> None:
    """Quantize with reserved pupil palette slots so the eye stays dark."""
    pupil0 = eye_pupil_mask(pupil_src) if pupil_src is not None else None
    reserved: list[tuple[int, int, int]] = []
    if pupil0 is not None and pupil0.any():
        cols, counts = np.unique(pupil_src[pupil0][:, :3], axis=0, return_counts=True)
        order = np.argsort(-counts)
        # keep a handful of exact blessed darks; rest snap to the top color
        for k in order[:8]:
            reserved.append(tuple(int(x) for x in cols[k]))

    ims = []
    pupil_masks = []
    for im in frames:
        if scale != 1:
            im = im.resize((W * scale, H * scale), Image.Resampling.NEAREST)
        ims.append(im.convert("RGBA"))
        if pupil0 is None:
            pupil_masks.append(None)
        elif scale != 1:
            m = Image.fromarray(pupil0.astype(np.uint8) * 255, "L")
            m = m.resize((W * scale, H * scale), Image.Resampling.NEAREST)
            pupil_masks.append(np.array(m) > 0)
        else:
            pupil_masks.append(pupil0)

    out_frames = []
    n_res = len(reserved)
    for im, pmask in zip(ims, pupil_masks):
        arr = np.array(im)
        rgb = arr[:, :, :3].copy()
        trans = arr[:, :, 3] == 0
        rgb[trans] = (255, 0, 255)
        # hide pupil from median-cut so maroon does not absorb it
        if pmask is not None:
            rgb[pmask] = (255, 0, 255)
        q_colors = 255 - n_res if n_res else 255
        q = Image.fromarray(rgb, "RGB").quantize(
            colors=max(2, q_colors), method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE
        )
        pal_list = list(q.getpalette())
        if len(pal_list) < 768:
            pal_list += [0] * (768 - len(pal_list))
        pal_list[255 * 3 : 255 * 3 + 3] = [255, 0, 255]
        # reserved slots just below transparency
        slot0 = 255 - n_res
        for i, col in enumerate(reserved):
            base = (slot0 + i) * 3
            pal_list[base : base + 3] = list(col)
        qp = np.array(q)
        qp[trans] = 255
        if pmask is not None and reserved:
            eye_rgb = arr[pmask][:, :3]
            # map each pupil pixel to nearest reserved blessed color
            ref = np.array(reserved, dtype=np.int16)
            d = np.abs(eye_rgb.astype(np.int16)[:, None, :] - ref[None, :, :]).sum(axis=2)
            qp[pmask] = slot0 + d.argmin(axis=1).astype(np.uint8)
        q2 = Image.fromarray(qp, "P")
        q2.putpalette(pal_list)
        out_frames.append(q2)

    out_frames[0].save(
        path,
        save_all=True,
        append_images=out_frames[1:],
        duration=DURATION_MS,
        loop=0,
        disposal=2,
        transparency=255,
        optimize=False,
    )



def _font(size: int) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", size
    )


def _label_panel(im: Image.Image, text: str) -> Image.Image:
    """256x384 sprite with a 22px caption bar on top."""
    bar_h = 22
    out = Image.new("RGBA", (W, H + bar_h), (12, 10, 14, 255))
    out.paste(im.convert("RGBA"), (0, bar_h))
    draw = ImageDraw.Draw(out)
    draw.rectangle((0, 0, W, bar_h), fill=(12, 10, 14, 255))
    draw.text((6, 3), text, fill=(255, 245, 194, 255), font=_font(13))
    return out


def make_pole_swatch() -> Image.Image:
    """Rose + space-dust + bloom chips, with a ring crop for the pole."""
    bar_h = 22
    panel = Image.new("RGBA", (W, H + bar_h), (18, 14, 22, 255))
    draw = ImageDraw.Draw(panel)
    draw.rectangle((0, 0, W, bar_h), fill=(12, 10, 14, 255))
    draw.text((6, 3), "poles  rose / dust / bloom", fill=(255, 245, 194, 255), font=_font(12))
    chips = [
        ((12, 32, W - 12, 78), (204, 73, 99), "#CC4963  rose KEEP"),
        ((12, 86, W - 12, 132), (124, 60, 165), "#7C3CA5  space-dust"),
        ((12, 140, W - 12, 186), (166, 84, 209), "#A654D1  bloom"),
        ((12, 194, W - 12, 230), (151, 80, 185), "#9750B9  high-sat"),
    ]
    for box, fill, label in chips:
        draw.rectangle(box, fill=fill + (255,))
        draw.text((18, box[1] + 12), label, fill=(255, 245, 194, 255), font=_font(12))
    ring_p = ROOT / "work/logo/ring_crop.png"
    if ring_p.exists():
        ico = Image.open(ring_p).convert("RGBA")
        fit = (W - 24, H + bar_h - 250)
        ico.thumbnail(fit, Image.Resampling.NEAREST)
        x = (W - ico.size[0]) // 2
        panel.paste(ico, (x, 242))
    return panel


def make_rim_swatch(arr: np.ndarray, title: str) -> Image.Image:
    """2x nearest crop of the head/shoulder rim."""
    bar_h = 22
    panel = Image.new("RGBA", (W, H + bar_h), (18, 14, 22, 255))
    draw = ImageDraw.Draw(panel)
    draw.rectangle((0, 0, W, bar_h), fill=(12, 10, 14, 255))
    draw.text((6, 3), title, fill=(255, 245, 194, 255), font=_font(13))
    crop = arr[24:168, 80:224]  # 144x144 head/shoulder
    im = Image.fromarray(harden_alpha(crop), "RGBA")
    im = im.resize((W - 16, W - 16), Image.Resampling.NEAREST)
    panel.paste(im, (8, 40), im)
    return panel


def _region_outer_hex(arr: np.ndarray, y0: int, y1: int) -> str:
    yy = np.mgrid[0:H, 0:W][0]
    m = _outer_halo_mask(arr) & (yy >= y0) & (yy < y1)
    return _hex_of(m, arr)


def make_split_panel(v8: np.ndarray, v9: np.ndarray) -> Image.Image:
    """Upper vs lower rim crops + hexes so the tail even-out is visible."""
    bar_h = 22
    panel = Image.new("RGBA", (W, H + bar_h), (18, 14, 22, 255))
    draw = ImageDraw.Draw(panel)
    draw.rectangle((0, 0, W, bar_h), fill=(12, 10, 14, 255))
    draw.text((6, 3), "peak rim  upper / tail", fill=(255, 245, 194, 255), font=_font(12))

    # head crop 144x144 -> ~112 wide; tail crop same
    crops = [
        (v8[24:168, 80:224], f"v8 head {_region_outer_hex(v8, 0, 120)}"),
        (v9[24:168, 80:224], f"v9 head {_region_outer_hex(v9, 0, 120)}"),
        (v8[240:384, 56:200], f"v8 tail {_region_outer_hex(v8, 240, H)}"),
        (v9[240:384, 56:200], f"v9 tail {_region_outer_hex(v9, 240, H)}"),
    ]
    # 2x2 of 112x112 nearest crops with 14px captions
    cell = 112
    gap = 8
    x0 = (W - (cell * 2 + gap)) // 2
    y0 = 36
    font_s = _font(10)
    for i, (crop, label) in enumerate(crops):
        col, row = i % 2, i // 2
        x = x0 + col * (cell + gap)
        y = y0 + row * (cell + 22)
        im = Image.fromarray(harden_alpha(crop), "RGBA")
        im = im.resize((cell, cell), Image.Resampling.NEAREST)
        panel.paste(im, (x, y), im)
        draw.text((x, y + cell + 2), label, fill=(255, 245, 194, 255), font=font_s)

    # hex summary
    v8u, v8l = _region_outer_hex(v8, 0, 192), _region_outer_hex(v8, 192, H)
    v9u, v9l = _region_outer_hex(v9, 0, 192), _region_outer_hex(v9, 192, H)
    draw.text(
        (10, H + bar_h - 44),
        f"v8 U {v8u}  L {v8l}",
        fill=(200, 170, 210, 255),
        font=font_s,
    )
    draw.text(
        (10, H + bar_h - 28),
        f"v9 U {v9u}  L {v9l}",
        fill=(220, 190, 230, 255),
        font=font_s,
    )
    return panel


def write_compare(idles: list[np.ndarray]) -> None:
    """Rest + peak: v8 vs v9, plus upper/lower rim split."""
    v8_rest_p = ROOT / "out_v8" / "idle_0.png"
    v8_peak_p = ROOT / "out_v8" / "idle_4.png"
    if not (v8_rest_p.exists() and v8_peak_p.exists()):
        print("compare skipped — missing v8 frames")
        return
    v8_rest = load_rgba(v8_rest_p)
    v8_peak = load_rgba(v8_peak_p)
    rest_row = [
        _label_panel(Image.fromarray(v8_rest, "RGBA"), "v8 rest  rose"),
        _label_panel(to_img(idles[0]), "v9 rest  rose"),
        make_pole_swatch(),
    ]
    peak_row = [
        _label_panel(Image.fromarray(v8_peak, "RGBA"), "v8 peak  dust"),
        _label_panel(to_img(idles[4]), "v9 peak  even dust"),
        make_split_panel(v8_peak, idles[4]),
    ]
    cw, ch = rest_row[0].size
    sheet = Image.new("RGBA", (cw * 3, ch * 2), (8, 6, 10, 255))
    for j, im in enumerate(rest_row):
        sheet.paste(im, (j * cw, 0))
    for j, im in enumerate(peak_row):
        sheet.paste(im, (j * cw, ch))
    sheet.save(OUT / "compare_v8_v9.png")


def _halo_family_mask(arr: np.ndarray) -> np.ndarray:
    """Rose or space-dust family, never cream / teal / ochre / eye."""
    rgb = arr[:, :, :3].astype(np.int16)
    a = arr[:, :, 3] > 0
    r, g, b = rgb[:, :, 0], rgb[:, :, 1], rgb[:, :, 2]
    cream = (r >= 240) & (g >= 230) & (b >= 180) & (r - g <= 20)
    teal = (g > r + 10) & (b > r) & (g > 80)
    ochre = (r > 180) & (g > 120) & (g < 210) & (b < 80)
    rose = (r > 170) & (g < 110) & (b > 60) & (b < 160) & (r > g + 70)
    violet = (b > 90) & (r > 70) & (g < 155) & (b + 12 > r) & (b > g + 8)
    return a & ~eye_disk_mask() & ~cream & ~teal & ~ochre & (rose | violet)


def _outer_halo_mask(arr: np.ndarray) -> np.ndarray:
    return _halo_family_mask(arr) & _n8(arr[:, :, 3] == 0)


def _hex_of(mask: np.ndarray, arr: np.ndarray) -> str:
    if not mask.any():
        return "------"
    mn = arr[:, :, :3][mask].astype(np.float32).mean(0)
    return f"#{int(round(mn[0])):02X}{int(round(mn[1])):02X}{int(round(mn[2])):02X}"


def halo_mean_hex(arr: np.ndarray, rose_m: np.ndarray) -> str:
    """Mean of currently-halo-family pixels (rose or space-dust)."""
    return _hex_of(_halo_family_mask(arr), arr)


def outer_halo_hex(arr: np.ndarray) -> str:
    """Mean of outer-silhouette halo pixels — the number Matt will see."""
    return _hex_of(_outer_halo_mask(arr), arr)


def main() -> None:

    OUT.mkdir(parents=True, exist_ok=True)
    blessed = load_rgba(BLESSED)
    assert blessed.shape == (H, W, 4)

    pupil = eye_pupil_mask(blessed)
    disk = eye_disk_mask()
    rose_m = rose_halo_mask(blessed)
    rose_pix = blessed[:, :, :3][rose_m].astype(np.float32)
    rose_mean = rose_pix.mean(0) if rose_m.any() else ROSE_POLE
    print(f"eye lock pupil={int(pupil.sum())} disk={int(disk.sum())}")
    print(
        f"halo poles rose=#CC4963 dust=#7C3CA5 bloom=#A654D1 hisat=#9750B9 "
        f"bloom+ =#AA56D6 "
        f"src_rose_mean=#{int(round(rose_mean[0])):02X}"
        f"{int(round(rose_mean[1])):02X}{int(round(rose_mean[2])):02X} "
        f"n={int(rose_m.sum())}"
    )

    weight = motion_weight()
    edge_w = edge_band_weight(blessed, weight)
    mix_edge_w = mix_edge_band_weight(blessed)

    # static space terms for the live mix (outer rim vs inner crease)
    rim0 = outer_rim_mask(blessed)
    dist0 = chamfer_distance(rim0, cap=8.0)
    # tight: only the outer ~1.5 px reads as "outer". Inner creases stay rose.
    outer_space = (1.0 - smoothstep(0.15, 1.55, dist0)).astype(np.float32)
    lit_space = smoothstep(88.0, 112.0, _luma(blessed[:, :, :3].astype(np.float32)))
    lit_space = lit_space.astype(np.float32)

    wmin, wmax = float(weight.min()), float(weight.max())
    head_w = float(weight[50, 198])
    core_w = float(weight[int(CORE_Y), int(CORE_X)])
    tail_w = float(weight[350, 128])
    left_w = float(weight[160, 24])
    print(
        f"weight min={wmin:.3f} max={wmax:.3f} head={head_w:.3f} "
        f"core={core_w:.3f} tail={tail_w:.3f} left={left_w:.3f}"
    )

    emax = float(edge_w.max())
    eye_e = float(edge_w[53, 198])
    cheek_e = float(edge_w[70, 200])
    core_e = float(edge_w[int(CORE_Y), int(CORE_X)])
    # pick a likely face-rim pixel
    rim = outer_rim_mask(blessed)
    rim_upper = rim & (np.mgrid[0:H, 0:W][0] < 100)
    if rim_upper.any():
        ys, xs = np.where(rim_upper)
        # face-right: max x
        k = int(np.argmax(xs))
        face_rim_e = float(edge_w[ys[k], xs[k]])
        face_xy = (int(ys[k]), int(xs[k]))
    else:
        face_rim_e, face_xy = 0.0, (0, 0)
    print(
        f"edge_w max={emax:.3f} eye={eye_e:.3f} cheek={cheek_e:.3f} "
        f"core={core_e:.3f} face_rim{face_xy}={face_rim_e:.3f} "
        f"upper_band={(edge_w > 0.05).sum()} "
        f"mix_edge={(mix_edge_w > 0.05).sum()} "
        f"mix_edge_tail={(mix_edge_w[240:] > 0.05).sum()}"
    )

    idles = []
    notes = []

    for i in range(N):
        p = float(pulse_amount(i))
        mix = OUTLINE_MIX_MAX * (1.0 - p)
        faded_v5 = fade_outline(blessed, mix)
        mix_field = halo_mix_field(i, outer_space, lit_space, mix_edge_w)
        tgt_field = halo_target_field(outer_space, lit_space)
        faded = mix_halo_live(faded_v5, rose_m, mix_field, tgt_field, outer_space)
        if i in (0, 4):
            ndiff = int(np.any(faded[~rose_m] != faded_v5[~rose_m], axis=1).sum())
            print(f"  f{i} non-rose identity diffs={ndiff} (want 0)")
        body, stamps = extract_specks(faded)

        dx, dy = displacement_fields(i, weight)
        edx, edy = edge_displacement_fields(i, edge_w)
        dx = dx + edx
        dy = dy + edy

        vis = body[:, :, 3] > 0
        mag = np.hypot(dx, dy)
        emag = np.hypot(edx, edy)
        notes.append(
            (
                i,
                p,
                mix,
                float(mag[vis].max()) if vis.any() else 0.0,
                float(emag[vis].max()) if vis.any() else 0.0,
                int(np.rint(dx[vis]).max()) if vis.any() else 0,
                int(np.rint(dx[vis]).min()) if vis.any() else 0,
                int(np.rint(dy[vis]).max()) if vis.any() else 0,
                int(np.rint(dy[vis]).min()) if vis.any() else 0,
                float(mix_field[rose_m].mean()) if rose_m.any() else 0.0,
                float(mix_field[rose_m].max()) if rose_m.any() else 0.0,
            )
        )

        warped = warp_backward(body, dx, dy)
        glow_src = load_rgba(GLOW_DIR / f"glow_{i}.png")
        idle = overlay_glow(warped, glow_src)

        for k, (stamp, y0, x0) in enumerate(stamps):
            sdx, sdy = speck_drift(i, k)
            paste_sprite(idle, stamp, y0 + sdy, x0 + sdx)

        # lock eye last — exact blessed darks; keep only cream/white 1px ticks
        tick = cream_tick_mask(glow_src)
        idle = lock_eye(idle, blessed, pupil)
        if tick.any():
            idle[tick] = glow_src[tick]

        idle = add_hard_dust_rim(idle, p)
        idle = harden_alpha(idle)
        idles.append(idle)
        to_img(idle).save(OUT / f"idle_{i}.png")

    sheet = Image.new("RGBA", (W * N, H))
    for i, arr in enumerate(idles):
        sheet.paste(to_img(arr), (i * W, 0))
    sheet.save(OUT / "idle.png")

    idle_imgs = [to_img(a) for a in idles]
    save_gif(idle_imgs, OUT / "preview.gif", scale=1, pupil_src=blessed)
    save_gif(idle_imgs, OUT / "preview_2x.gif", scale=2, pupil_src=blessed)

    # rest / peak stills for chat (1x + 2x nearest)
    rest_i, peak_i = 0, 4
    to_img(idles[rest_i]).save(OUT / "idle_still.png")
    to_img(idles[rest_i]).resize((W * 2, H * 2), Image.Resampling.NEAREST).save(
        OUT / "idle_still_2x.png"
    )
    to_img(idles[peak_i]).save(OUT / "idle_peak.png")
    to_img(idles[peak_i]).resize((W * 2, H * 2), Image.Resampling.NEAREST).save(
        OUT / "idle_peak_2x.png"
    )
    write_compare(idles)

    print("wrote", OUT)
    print("poles rose=#CC4963  dust=#7C3CA5  bloom=#A654D1  hisat=#9750B9  bloom+=#AA56D6")
    for row in notes:
        i, p, mix, mag, emag, dxh, dxl, dyh, dyl, hmean, hmax = row
        print(
            f"  f{i}: pulse={p:.3f} outline_mix={mix:.3f} "
            f"halo_mix mean={hmean:.3f} max={hmax:.3f} "
            f"halo={halo_mean_hex(idles[i], rose_m)} "
            f"outer={outer_halo_hex(idles[i])} "
            f"travel_max={mag:.2f} edge_max={emag:.2f} "
            f"dx=[{dxl:+d},{dxh:+d}] dy=[{dyl:+d},{dyh:+d}]"
        )
    print(
        f"MEASURE rest_outer={outer_halo_hex(idles[0])} "
        f"peak_outer={outer_halo_hex(idles[4])} "
        f"rest_halo={halo_mean_hex(idles[0], rose_m)} "
        f"peak_halo={halo_mean_hex(idles[4], rose_m)}"
    )
    print(
        f"MEASURE peak_outer_upper={_region_outer_hex(idles[4], 0, 192)} "
        f"peak_outer_lower={_region_outer_hex(idles[4], 192, H)} "
        f"peak_outer_head={_region_outer_hex(idles[4], 0, 120)} "
        f"peak_outer_tail={_region_outer_hex(idles[4], 240, H)}"
    )
    v8p = ROOT / "out_v8" / "idle_4.png"
    if v8p.exists():
        v8_peak = load_rgba(v8p)
        print(
            f"v8ref peak_outer={outer_halo_hex(v8_peak)} "
            f"upper={_region_outer_hex(v8_peak, 0, 192)} "
            f"lower={_region_outer_hex(v8_peak, 192, H)} "
            f"head={_region_outer_hex(v8_peak, 0, 120)} "
            f"tail={_region_outer_hex(v8_peak, 240, H)}"
        )

    for i, arr in enumerate(idles):
        alphas = set(arr[:, :, 3].flatten().tolist())
        assert alphas <= {0, 255}, alphas
        print(f"  idle_{i} opaque={(arr[:,:,3]>0).sum()}")

    # verify locked eye vs blessed
    samples = [(53, 198), (53, 197), (52, 198), (54, 198), (50, 198), (56, 198)]
    print("eye hex vs blessed (PNG):")
    bpix = {xy: tuple(blessed[xy].tolist()) for xy in samples}
    for xy, bp in bpix.items():
        print(f"  blessed {xy} #{bp[0]:02X}{bp[1]:02X}{bp[2]:02X}")
    ok = True
    for i, arr in enumerate(idles):
        for xy in samples:
            fp = tuple(arr[xy].tolist())
            bp = bpix[xy]
            match = fp == bp
            ok = ok and match
            flag = "OK" if match else "DIFF"
            print(
                f"  f{i} {xy} #{fp[0]:02X}{fp[1]:02X}{fp[2]:02X} "
                f"blessed #{bp[0]:02X}{bp[1]:02X}{bp[2]:02X} {flag}"
            )
        # whole pupil exact
        mism = int((arr[pupil] != blessed[pupil]).any(axis=1).sum())
        print(f"  f{i} pupil mismatches={mism}/{int(pupil.sum())}")
        ok = ok and mism == 0
    print("eye lock", "PASS" if ok else "FAIL")

    # cream / teal / ochre must stay off the rose remap
    rgb0 = blessed[:, :, :3].astype(np.int16)
    a0 = blessed[:, :, 3] > 0
    r0, g0, b0 = rgb0[:, :, 0], rgb0[:, :, 1], rgb0[:, :, 2]
    cream_m = a0 & (r0 >= 240) & (g0 >= 230) & (b0 >= 180) & (r0 - g0 <= 20) & ~rose_m & ~disk
    teal_m = a0 & (g0 > r0 + 10) & (b0 > r0) & (g0 > 80) & ~rose_m
    ochre_m = a0 & (r0 > 180) & (g0 > 120) & (g0 < 210) & (b0 < 80) & ~rose_m
    print(
        f"identity masks cream={int(cream_m.sum())} teal={int(teal_m.sum())} "
        f"ochre={int(ochre_m.sum())} (pre-warp source)"
    )
    probes = {
        "cream": (84, 160),
        "teal": (161, 107),
        "ochre": (180, 125),
        "eye": (53, 198),
    }
    v5_0p = ROOT / "out_v5" / "idle_0.png"
    v5_4p = ROOT / "out_v5" / "idle_4.png"
    v5_0 = load_rgba(v5_0p) if v5_0p.exists() else None
    v5_4 = load_rgba(v5_4p) if v5_4p.exists() else None
    print("identity probes vs v5 (same motion; interiors should match):")
    for name, xy in probes.items():
        y, x = xy
        t8_0 = tuple(int(v) for v in idles[0][y, x])
        t8_4 = tuple(int(v) for v in idles[4][y, x])
        bles = tuple(int(v) for v in blessed[y, x])
        msg = (
            f"  {name} {xy} blessed=#{bles[0]:02X}{bles[1]:02X}{bles[2]:02X} "
            f"v9rest=#{t8_0[0]:02X}{t8_0[1]:02X}{t8_0[2]:02X} "
            f"v9peak=#{t8_4[0]:02X}{t8_4[1]:02X}{t8_4[2]:02X}"
        )
        if v5_0 is not None and v5_4 is not None:
            v50 = tuple(int(v) for v in v5_0[y, x])
            v54 = tuple(int(v) for v in v5_4[y, x])
            m0 = "MATCH" if t8_0 == v50 else "DIFF"
            m4 = "MATCH" if t8_4 == v54 else "DIFF"
            msg += f" v5rest={m0} v5peak={m4}"
        print(msg)


if __name__ == "__main__":
    main()
