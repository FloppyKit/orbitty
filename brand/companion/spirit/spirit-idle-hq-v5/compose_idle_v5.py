#!/usr/bin/env python3
"""Orbitty companion glow idle v5 — art compositor.

v4 motion (teal pulse, upper-rim edge-wave, tail wave, two specks)
with the eye locked to talk_v4_bright.png.

After warps / outline-mix / glow, dark eye pixels are copied
exactly from the blessed still. Outline-fade never includes the
eye disk. GIF palette reserves the pupil colors so quantization
cannot remap the eye toward maroon/red.
"""
from __future__ import annotations

from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path("/workspace/orbitty-spirit")
BLESSED = ROOT / "talk_v4_bright.png"
GLOW_DIR = ROOT / "out"  # untranslated v1 core pulse
OUT = ROOT / "out_v5"

W, H = 256, 384
N = 8
DURATION_MS = 140

CREAM = np.array([255, 245, 194], dtype=np.float32)
OUTLINE_MIX_MAX = 0.44

# Spatial lock / travel
Y_HEAD = 78.0
Y_TAIL = 328.0
CORE_Y, CORE_X = 188.0, 134.0
CORE_R0, CORE_R1 = 28.0, 72.0

# Eye cluster — never shear these pixels; stamp after compose
EYE_Y, EYE_X, EYE_R = 53.4, 197.9, 11.0
EYE_DARK_MAX = 80  # dark brown/black pupil only

# Upper-half edge wave: full above Y_EDGE_FULL, gone by Y_EDGE_FADE
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


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    blessed = load_rgba(BLESSED)
    assert blessed.shape == (H, W, 4)

    pupil = eye_pupil_mask(blessed)
    disk = eye_disk_mask()
    print(f"eye lock pupil={int(pupil.sum())} disk={int(disk.sum())}")

    weight = motion_weight()
    edge_w = edge_band_weight(blessed, weight)

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
        f"upper_band={(edge_w > 0.05).sum()}"
    )

    idles = []
    notes = []

    for i in range(N):
        p = float(pulse_amount(i))
        mix = OUTLINE_MIX_MAX * (1.0 - p)
        faded = fade_outline(blessed, mix)
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

    print("wrote", OUT)
    for i, p, mix, mag, emag, dxh, dxl, dyh, dyl in notes:
        print(
            f"  f{i}: pulse={p:.3f} outline_mix={mix:.3f} "
            f"travel_max={mag:.2f} edge_max={emag:.2f} "
            f"dx=[{dxl:+d},{dxh:+d}] dy=[{dyl:+d},{dyh:+d}]"
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


if __name__ == "__main__":
    main()
