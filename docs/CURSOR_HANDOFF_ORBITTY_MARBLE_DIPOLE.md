# Cursor Ultra — GO-ORBITTY-MARBLE-DIPOLE

**Model:** `kimi-k3-max` · **one writer**
**Gate:** Matt **GO-ORBITTY-MARBLE-DIPOLE** via Pixel Artist — **issued 2026-08-28**. Code this. Part 1f of 2. No wells.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_MARBLE_DIPOLE_LOCK_v0.md` + this handoff
**Part:** **1f of 2.** Revises 1e join + oval. Same `marbleDrift` toggle. **No wells.**

Matt 2026-08-28: waltz is much better. More oval. Smoother together. Join should not rush — two like magnets, slow then a little faster then rotate. Orbit = 360° of two magnets pushed together from outside and pushed apart because they repel.

## Ship

Rewrite the **inner pair** in `drift_pass`. Keep moons, keep-in, drag pin, Reduce Motion, every-tick `drift_tick`, hysteresis 220/280.

**Oval**

- `OVAL_RATIO` **0.75 → 0.55** (more oval, ~3×2). Still one hashed in-plane rotation. Not a pancake, not XY/XZ/YZ.

**Rigid dumbbell (this is the sync)**

- Inner two are **one stick**: always opposite through the frozen home. Never two independent polar lerps onto slots.
- On recapture / first join: `home` = live midpoint (once). Stick angle = current vector. Stick length = current separation.
- Target length = `2 * inner_r` where `inner_r` is half join-separation clamped **24–40pt**, then **floor** so length ≥ `2 * MARBLE_RADIUS + 8` (they repel; they do not kiss).
- Approach **~6s**, ease-**in** (`t^2` or cubic-in — **slow first**, slightly faster as they close). Not 1.5s smoothstep. Not ease-out.
- While shortening, **ω ramps 0 → waltz ω** (linear in the same 6s ease-in). They start turning before they arrive. After blend ≥ 1, full shared θ on the oval as today (period 75–90s).
- Pass / fake Z / moons / keep-in / isolates / drag-pin / orbit_restore / worlds_dirty: keep.

**Do not**

- Pull either marble **at** the other (no lerp toward neighbor, no lerp toward home as a point). Move both ends of the stick.
- Springs, velocity, mass, Kepler.
- Change join/leave distances (220/280 stay).
- Home creep, nearest-expanded host, hashed pancakes.

Settings tooltip: waltz on a fuller oval; nearby marbles ease into the dance like magnets that never quite meet. Same key.

`habitat_stub` no-op.

## Files

| File | Job |
|------|-----|
| `src-tauri/src/habitat_mac.rs` | Inner pair = dumbbell + longer ease-in; `OVAL_RATIO = 0.55` |
| `src/lib/SettingsPage.svelte` | Tooltip only |

## Forbidden

Wells, poles, STARS, Talk, MCP, Jacob, n-body, springs, default-on, 1.5s rush lerp, independent spirals, per-marble clamp, `tauri dev`, STAMP_*

## Dogfood

1. Tray **Quit** → only `~/Applications/Orbitty.app` → AX re-grant
2. **Marbles drift** on
3. Two far apart: sit. Drag them toward each other past ~220pt: they should **slowly** become a pair, start to turn, then waltz a **fuller oval** — never a snap/rush
4. Once waltzing they stay opposite and read as one object
5. Third still a moon. Isolates sit. Drag wins. Expanded does not steal
