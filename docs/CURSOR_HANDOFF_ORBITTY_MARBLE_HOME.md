# Cursor Ultra — GO-ORBITTY-MARBLE-HOME

**Model:** `kimi-k3-max` · **one writer**
**Gate:** Matt **GO-ORBITTY-MARBLE-HOME** via Pixel Artist — **issued 2026-08-28**. Code this. Part 1d of 2. No wells.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_SOLAR_NEST_TALK_v0.md` (Marble fidget) + this handoff
**Part:** **1d of 2.** Revises 1c. Same `marbleDrift` toggle. **No wells.** Part 2 remains `GO-ORBITTY-GRAVITY-POLES`.

Matt 2026-08-28 eyes-on of `GO-ORBITTY-MARBLE-HOST` (signed 1:03 AM): **significantly better**, but two marbles still **dance the screen** too much. Want them **mostly to themselves, swirling around each other, with just a very slight drift**. Feels **anchored to the bottom-left** of the screen.

## Why 1c still walks / pins a corner

`drift_pass` in `habitat_mac.rs`:

1. **Host steals the desk.** Any expanded planet on the screen, **no distance cap**, becomes the ring at 80–200pt. That’s a stroll around the planet, not a pair swirling in place.
2. **No-host fallback uses the live centroid** every tick. The pair’s midpoint is their current positions → the ring **walks**.
3. **Per-marble `visibleFrame` clamp** on the 1b path. Cocoa `visibleFrame.origin` is the **bottom-left** of the usable screen. Independent clamp flattens a ring into a slide toward that origin.

This GO: **freeze a home**, swirl around it, tiny drift of the home, clamp the **home**, never each marble. Expanded planets **do not** become the ring (1c host-as-center is **off** until wells).

## Ship

Rewrite the hosted + rogue branches of `drift_pass`. Same `marbleDrift` (default false). No new Settings key.

- **Home.** When a cluster of ≥2 same-screen marbles forms (join ≤ 220pt, leave ≥ 280pt), capture `home = midpoint` (pair) / centroid (3+) **once** from live positions at join. Store on the in-memory orbit state. Do **not** recompute home from live x/y every tick.
- **Swirl.** Members equally spaced on a ring around **that home**. Radius = half their join separation, clamp **24–48pt** (they hug each other, not a 200pt planet ring). Shared θ, hashed 60–90s. Hashed XY/XZ/YZ + fake Z (scale 0.88–1.12, z-order among members only). Panel size stays 96pt.
- **Slight drift.** Home may walk a **tiny** ellipse around the captured point: radius **4–10pt**, period **180–360s**. That’s the only translation of the cluster. Not a second orbit of the marbles.
- **Screen.** If the ring would leave `visibleFrame` (pad ≥ marble + 8), **nudge home inward** and/or shrink radius down to 24pt so the **whole ring** stays on-screen. Never `clamp` each marble’s x/y independently. Never use `(0,0)` or `visibleFrame.origin` as a target / fallback home.
- **Expanded sits.** `is_marble == false` is **not** a host in this GO. Do not assign marbles to the nearest planet. (Wells later win as host; not this file.)
- **Isolates sit.** Lone marble does not orbit a ghost home. Drag wins (~2s pin), then rejoin eases in **polar around home** (~1.5s smoothstep) — never a straight lerp at a neighbor.
- **Reduce Motion** stills + restores size (`orbit_restore` already). Persist `worlds.json` x/y via `worlds_dirty` 10Hz. Do not persist home / θ / plane / scale.
- `habitat_stub` no-op.

## Files

| File | Job |
|------|-----|
| `src-tauri/src/habitat_mac.rs` | `MarbleOrbit` gains frozen `home` (+ tiny drift phase). `drift_pass`: no host pick; cluster home + 24–48pt ring + 4–10pt home drift; unit clamp. |
| `src/lib/SettingsPage.svelte` | Tooltip only if it still says “nearest open planet” — revert to swirl / slight drift. Same key. |

## Forbidden

- Wells / poles / STARS / Talk / MCP / Jacob
- n-body, mass, velocity, screensaver, cursor-chase, default-on
- Nearest-expanded as ring center (that was 1c; Matt walked it back)
- Live centroid as the ring (that walks)
- Per-marble screen clamp (that is the bottom-left magnet)
- Radial / straight pull
- `tauri dev`, STAMP_*

## Dogfood

1. Tray **Quit** → only `~/Applications/Orbitty.app` → AX re-grant
2. Settings → Appearance → **Marbles drift** on
3. Two nearby marbles: swirl around **each other** where you left them; cluster may creep a few points over minutes, not march the desk
4. Expanded planet on the same screen does **not** steal them
5. Isolates sit; drag still wins
6. Nothing hugs the Dock / bottom-left corner unless you put them there
