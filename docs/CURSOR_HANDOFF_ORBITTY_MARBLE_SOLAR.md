# Cursor Ultra — GO-ORBITTY-MARBLE-SOLAR

**Model:** `kimi-k3-max` · **one writer**
**Gate:** Matt **GO-ORBITTY-MARBLE-SOLAR** via Pixel Artist — **issued 2026-08-28**. Part 1g. No wells.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_MARBLE_SOLAR_LOCK_v0.md` + this handoff
**Part:** **1g of 2.** Rewrite `drift_pass`. No wells.

Matt after 1f dipole (signed 9:24 AM): still **water-blobs**, not planets. Magnetic / solar-system bodies. Gravity poles **are the sun** — movable, **part 2**. This GO is the motion those wells will host.

## Why

Dipole is a **tiny rigid stick** (inner r 24–40pt on 18pt marbles) around the pair itself + a size-pulse fake Z. That reads as grapes glued, throbbing. 1c (host) stole with a small ring and walked. Kill both recipes.

## Picture

Each marble on **its own large track** around a **sun that is not them**. They do not seek each other. Never kiss. Even slow speed. Fake Z = **z-order at crossing only** — scale stays 1.0.

## Sun this GO (Matt closed)

1. Nearest **expanded planet** on that screen (`is_marble == false`, not collapsed). Planet **sits**.
2. Else **invisible frozen barycenter** of the nearby marble cluster. Frozen at join. **Large independent rings**, not a stick.

Wells win over both — **not this GO**. Empty well list is correct here.

## Law

1. **Marble only.** Expanded never translates.
2. Same gate: `marbleDrift` off **or** `h.reduce_motion` → sit. No second toggle.
3. **Drag wins.** After drop, ~2s pin, then may rejoin **their track** (ease onto the oval — not toward another marble).
4. Move NSPanels via existing `anim_tick` / `drift_tick` every ~16ms.
5. **No wells / poles / Drop well / nest / n-body / springs / velocity / Kepler / mass / screensaver / cursor-chase.**
6. No Talk / Speech / Handy / MCP / ctl / Jacob / STARS / default-on / `tauri dev` / `STAMP_*`.
7. Dogfood **only** `~/Applications/Orbitty.app` after tray Quit.

## Motion

Rewrite inner-pair **stick** + moon concentric-oval + scale-throb.

**Host = expanded planet (same screen):**
- Sun = that planet’s panel center. Frozen while it remains host.
- Each marble on that screen that is in range gets **its own** radius + phase, hashed from id.
- Radius **90–160pt** (planet disc is 72pt — they clear it). Not 24–40pt.
- Mild oval, almost round: `OVAL_RATIO` **0.88** (not 0.55 dipole, not a pancake). One hashed in-plane rotation per **sun**, not per pair.
- Period **90–140s**, even speed along the path (constant dθ; do not Kepler).
- Host pick: nearest expanded on the same screen. Switch only if a new host is **≥80pt closer** (hysteresis). Isolates with **no** expanded and **no** cluster sit.

**Fallback = barycenter (no expanded on that screen):**
- Cluster marbles with existing 220 join / 280 leave hysteresis.
- On recapture: `home` = live centroid **once**. Do not walk the home.
- Each member: independent r **70–140pt** hashed from id, independent phase0 hashed from id. **Not** opposite ends of a stick. **Not** one shared θ.
- Same oval / period / even speed / z-order pass.

**Join / leave:**
- Entering a sun: ease onto **that marble’s** target (home + oval offset) over ~6s ease-in. Do **not** lerp toward another marble.
- Leaving: sit (isolate) or ease onto the new sun’s track. No rush, no kiss.
- Lone marble + expanded host: still orbits that planet. Lone marble, no host: sits.

**Pass:** `orderFront` / z-order from the oval’s fake-depth **coordinate** (sin of θ is enough). **Do not** pulse `marble_scale` / sprite scale. Scale = 1.

**Keep:** drag-pin `DRIFT_PIN_SECS`, `orbit_restore` on drag end, `worlds_dirty` 10Hz, keep-in clamps the **home/sun** (never yank a marble radially to the screen edge), every-tick drift, `habitat_stub` no-op.

**Kill:** `stick_a0` / `stick_r0` / `stick_c0` / `DIPOLE_APPROACH` as pair-shortening / `ORBIT_R_MIN/MAX` 24–40 / scale-throb / inner-pair vs moon split / hashed XY/XZ/YZ pancakes.

## Settings

Same **Marbles drift** key, default false. Tooltip: nearby marbles take slow wide tracks around the nearest open planet; if none, around a quiet midpoint. They do not chase each other.

## Files

- `src-tauri/src/habitat_mac.rs` — `drift_pass` + cluster capture + constants
- `src/lib/SettingsPage.svelte` — tooltip only
- `habitat_stub.rs` — no-op

When done: `./scripts/install-stable.sh`. Summarize. Do **not** write `STAMP_*`.
