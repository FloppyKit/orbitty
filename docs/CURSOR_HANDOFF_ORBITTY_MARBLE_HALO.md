# Cursor Ultra — GO-ORBITTY-MARBLE-HALO

**Model:** `kimi-k3-max` · **one writer**
**Gate:** Matt **GO-ORBITTY-MARBLE-HALO** via Pixel Artist — **issued 2026-08-28**. Part 1h of 2. No wells.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_MARBLE_HALO_LOCK_v0.md` + `ORBITTY_MARBLE_HALO_TALK_v0.md` + this handoff

Matt on 1g (signed 11:50): **much much cooler**, but (1) hitch `1 2 3 4 stop` all marbles together, (2) round ring still covers the open planet's face. Closed 12:10: **3 widths · hitch in this GO · barycenter left at 1g.**

## Why

1g hosted track is a near-circle `HOST_R` 90–160 around a 72pt planet. Min r **kisses** the disc (`90 − 72 − 18 = 0`). A round ring **must** sweep the middle of the face every lap. Fake Z only reorders marbles vs marbles — the host never paints on top — so nothing tucks behind.

Hitch is the clock, not θ. Periods are already 90–140s hashed per marble; they would not stall *in unison* from phase. `drift_pass` sets `worlds_dirty` every move; `cursor_tick` (`n % 6`, ~10Hz) **sync-writes `worlds.json`** on the same 16ms main-thread path as `setFrameOrigin`. That is the 4-beat.

## Picture

> The open planet is a window. Moons may hide **behind** it freely. They may not sit on its **face**. The only time a moon is in front of its pixels is a brief graze of the left or right limb.

## Law

1. **Marble only** translates under drift. Expanded never moves.
2. Same gate: `marbleDrift` off **or** `h.reduce_motion` → sit. No second toggle.
3. **Drag wins.** ~2s pin, then ease onto **that marble's** current target (the `r(θ)` point) — not toward a neighbor.
4. Motion stays on the ~16ms `drift_tick` path once clustered. Do **not** put motion back on `n % 3`.
5. **No wells / poles / Drop well / nest / n-body / springs / velocity / Kepler / mass / screensaver / cursor-chase / CADisplayLink.**
6. No Talk / Speech / Handy / MCP / ctl / Jacob / STARS / rail-house / default-on / `tauri dev` / `STAMP_*`.
7. Dogfood **only** `~/Applications/Orbitty.app` after tray Quit.

## A. Hitch — take JSON off the 16ms path

Today: `drift_pass` writes `w.x`/`w.y` **and** sets `h.worlds_dirty = true` every orbiting tick. `cursor_tick` (~10Hz) then `save_worlds`. That stalls every marble together.

Ship:

- Keep writing live `w.x`/`w.y` in memory (restore / keep-in still work).
- **Do not** set `worlds_dirty` from the every-tick orbit path.
- Persist marble live x/y at **~1Hz** (a `last_orbit_save` Instant is enough) **and** on pin / drop / toggle / quit (existing `persist_position` on drop stays).
- `cursor_tick` 10Hz `save_worlds` **stays** for scroll-zoom / menu mutations that already set `worlds_dirty`. Do not disable that path.
- `orderFrontRegardless` only when the ranking **actually flips** (already) — now the ranking includes the host (below). Do not orderFront every tick.

Forbidden hitch “fixes”: Kepler, dropping motion to 20Hz (that *is* a stop), CADisplayLink, per-marble clocks that still share a 10Hz JSON save.

## B. Hosted track — not a constant-r oval

Barycenter (no expanded in range): **do not touch** 1g numbers. `BARY_R` 70–140, `OVAL_RATIO` 0.88, join/leave **220 / 280**, independent phase/period/dir, frozen home, even `dθ`.

Host = expanded planet on that screen. Still: planet sits; independent phase0 / dir / period 90–140s hashed from id; even `dθ`; join ease ~6s ease-in onto **that** marble's target; scale 1.0; drag-pin; host hysteresis ≥80pt; keep-in clamps the **sun** (nudge sun first; never radial-yank a marble to the screen edge).

Replace `HOST_R_MIN/MAX` 90–160 constant oval.

**Widths:** marble radius 18 → width = diameter = **36pt**. Three widths = **108pt** gap from planet **edge**.

```
HOST_FRONT_R = EXPANDED_RADIUS + 3*(2*MARBLE_RADIUS) + MARBLE_RADIUS
             = 72 + 108 + 18 = 198pt   // marble center, front
```

Polar `r(θ)` around the host, after the existing per-sun hashed in-plane rotation (keep `sun_rot` — fake depth is the oval's sin, not screen-down):

Let `s = sin(θ)`, `c = cos(θ)` after that rotation (same θ that 1g used for fake Z).

- **Front** (`s > 0`): almost always `r = HOST_FRONT_R` (198). Projected marble disc must **not** overlap the planet disc, except:
- **Limb graze (rare, front only):** near the left/right limbs (`|c|` high, `s` small positive). Briefly dip toward `HOST_GRAZE_R ≈ EXPANDED_RADIUS + MARBLE_RADIUS − 6` (~84pt) so a **sliver** of the side overlaps. Never the middle 50% of the disc. Use a **sharp** window (high power on `(1−s)` × `|c|`) so graze is a small fraction of the period — not half the lap.
- **Back** (`s < 0`): `r` shrinks toward `HOST_BACK_R ≈ 80pt` so the marble **intersects the silhouette**. Overlap is wanted. Any direction.
- Blend at the equator so `r` doesn't pop.
- Hosted path uses this `r(θ)` **instead of** `OVAL_RATIO` 0.88. Do not also squash — a 0.88 squash would pull the front halo onto the face.
- Drawn scale stays **1.0**. No throb.

**Host join / leave** (expanded sun only): **280 / 360**. Today's 220/280 will drop a marble sitting at 198. Bary stay 220/280.

**Keep-in:** prefer sun-nudge over `rscale`. If you must scale, do not pull front `r` below `EXPANDED_RADIUS + MARBLE_RADIUS + 36` (one width) — face-cover law dies first.

## C. Z-order includes the host

1g: marbles vs marbles via `sin θ`; host planet is not in the stack.

Ship, per host group:

1. Back marbles (`s < 0`) farthest first
2. **The expanded host planet**
3. Front marbles (`s > 0`) — limb graze on top so the sliver is in front of the pixels

`orderFrontRegardless` back → host → front, **only when that sequence changes**. Do not reorder other expanded planets, other screens, or other apps.

Behind = overlap + planet painted on top. In-front-at-198 does not cover the face either way; graze must be above the host.

## Settings

Same **Marbles drift** key, default false. Tooltip: when a planet is open, nearby marbles stay about three marble-widths off its face, may tuck behind it, and only rarely graze a side. If none is open, they take slow rings around a quiet midpoint. They do not chase each other. Reduce Motion stills.

## Files

- `src-tauri/src/habitat_mac.rs` — `drift_pass` hosted `r(θ)` + host z-order + orbit persist cadence. Constants. Host join/leave 280/360.
- `src/lib/SettingsPage.svelte` — tooltip only
- `habitat_stub.rs` — no-op

## Keep from 1g

`marbleDrift` default false, `JOIN_EASE_SECS` 6, `DRIFT_PIN_SECS` 2, `HOST_SWITCH_CLOSER` 80, bary 70–140 / 0.88 / 220/280, every-tick `drift_pass` once clustered, `orbit_restore`, keep-in clamps sun, isolates with no sun sit, lone marble + expanded still orbits, `habitat_stub` no-op.

## Kill / do not reintroduce

`HOST_R_MIN/MAX` 90–160 as a constant hosted oval. 10Hz `worlds_dirty` from the orbit path. Dipole stick, 24–40pt, scale-throb, Kepler, n-body, wells, poles, Drop well, `STAMP_*`.

When done: `./scripts/install-stable.sh`. Summarize. Do **not** write `STAMP_*`.
