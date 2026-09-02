# Cursor Ultra / Pixel — GO-ORBITTY-MARBLE-RIM

**Gate:** issued 2026-08-28. Phrase `GO-ORBITTY-MARBLE-RIM`.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**SoT:** `~/.buzz/PLANS/ORBITTY_MARBLE_RIM_LOCK_v0.md`
**Part 1j of 2.** Rewrites hosted path + hitch clock after glass eyes-on rejected. No wells.

## Why glass failed

Hitch **1, 2, 3, pause** — `n % 6 == 0` **skipped** `drift_pass`; that hole *is* the pause. Large disc swallows — `HOST_FRONT_R` baked to 72pt while `planet.radius` grows; back accordion to `HOST_BACK_R` 80pt is inside a zoomed chip. Weird orbits = radial accordion, not a ring. Keep: default-size face-cluster is gone.

## Hitch

1. Delete the motion skip (`n % 6 == 0` return in `drift_tick`). `drift_pass` runs every motion turn once clustered.
2. **CVDisplayLink** (Core Video, macOS) is the motion clock for `setFrameOrigin` / `drift_pass`. Not CADisplayLink. Create on start; stop on habitat stop. Output callback: if not main, hop to main, then `drift_pass` only. Do **not** run `cursor_tick`, `save_worlds`, or the full hit-test on the DisplayLink callback.
3. Existing 16ms thread keeps `drag_tick`, `anim_tick`, `cursor_tick` (10Hz save for scroll-zoom/menu dirty). It must **not** also call `drift_pass` if DisplayLink is running (two clocks = jitter).
4. If `CVDisplayLinkCreate*` fails: keep 16ms `drift_pass` every tick, **never skip**. Log once. Stop. Do not invent a third clock.

Persist: orbit path still never sets `worlds_dirty`. Live x/y in memory. Persist on pin/drop/toggle/quit only.

## Hosted track

Kill `host_r(s)`, `HOST_BACK_R` accordion, `HOST_GRAZE_R`, `halo_offset` r(θ) dip. Hosted path is a **circle**:

```
r = live_planet.radius + 3.0 * (2.0 * MARBLE_RADIUS) + MARBLE_RADIUS
```

`live_planet.radius` = scroll-zoomed drawn disc (`expanded_radius` / world zoom), **not** the `EXPANDED_RADIUS` 72 constant. Bigger chip → bigger ring, still ~3 marble-widths off the **edge**. Even dθ. Independent hashed phase/period/dir. Scale 1.0. Track around the **planet center**, not a wobble.

Keep-in: nudge the sun; never pull hosted r below `live_radius + MARBLE_RADIUS + 2*MARBLE_RADIUS` (one width floor). Prefer sun-nudge over rscale.

## Collider — law, not a window

Do **not** create a second NSPanel / “glass window.”

For each expanded host, `R_disc = live radius`. Marble center must satisfy `dist >= R_disc + MARBLE_RADIUS`. If unconstrained pos is closer, push out along `(pos - sun)` to that min distance (go around the rim, never through the chip). Run this after the track/join pos is computed. Also when a **bary** marble would clip an expanded disc.

Join ease: **polar along that marble’s ring** (ease θ or ease onto the circle). **Kill cartesian lerp** through the chip (that *is* going straight through). 6s ease-in stays.

Graze onto the face: **do not**. 3-width ring is the happy path.

## Z

Keep per-marble boolean vs **own** host, equator deadband, `orderWindow:relativeTo:` only when the bit flips (or join). No group `orderFrontRegardless`. No r shrink to fake a crescent. Glass wobble disc **delete** (the collider is the real disc, un-wobbled).

## Keep

Host join/leave **280/360**. Bary join/leave **220/280**, r 70–140, `OVAL_RATIO` 0.88, frozen home — **DO NOT TOUCH** bary numbers. marbleDrift default false. Drag-pin 2s. Host hysteresis 80pt. Isolates sit. Lone+expanded still orbits. `habitat_stub` no-op. Reduce Motion stills.

Settings tooltip only: nearby marbles ring an open planet about three marble-widths off its **current** size and go around the disc instead of through it. If none is open, slow rings around a quiet midpoint.

## Forbidden

Second NSPanel for the collider. Drawing the disc/bubble. `HOST_BACK_R` accordion. `HOST_GRAZE_R`. Skipping `drift_pass`. Two motion clocks at once. CADisplayLink. wells / poles / Drop well / STARS / Talk / MCP / Jacob / n-body / springs / Kepler / scale-throb / dipole / `tauri dev` / `STAMP_*` / rail house / picker.

When done: `./scripts/install-stable.sh`. Tray Quit, ONLY `~/Applications/Orbitty.app`. Summarize hitch clock (DisplayLink or fallback), live r formula, collider. Do **not** write `STAMP_*`. Do **not** claim hitch is gone — Matt eyes-on.
