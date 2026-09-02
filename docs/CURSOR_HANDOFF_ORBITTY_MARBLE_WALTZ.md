# Cursor Ultra — GO-ORBITTY-MARBLE-WALTZ

**Model:** `kimi-k3-max` · **one writer**
**Gate:** Matt **GO-ORBITTY-MARBLE-WALTZ** via Pixel Artist — **issued 2026-08-28**. Code this. Part 1e of 2. No wells.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_MARBLE_WALTZ_LOCK_v0.md` + this handoff
**Part:** **1e of 2.** Revises 1d path recipe. Same `marbleDrift` toggle. **No wells.** Part 2 remains `GO-ORBITTY-GRAVITY-POLES`.

Matt 2026-08-28: 1d is way better; still jerky; ellipses not quite right. Stop throwing darts. Locked **Waltz, mild oval, yes pass, home frozen.** 3+ = inner pair + moons.

## Ship

Rewrite `drift_pass` (and the tick rate). No new Settings key.

**Pair waltz**

- Cluster ≥2, same screen, join ≤ 220pt, leave ≥ 280pt (keep hysteresis).
- Home = midpoint of the **inner pair**, captured once at join. Frozen. **No** `home_drift` ellipse. **No** live centroid. **No** nearest-expanded host.
- Inner two equally opposite on a **mild oval** (~4×3, axis ratio ~0.75). One hashed orientation for the whole cluster (stable while membership of the *inner pair* is stable) — rotate the oval in the plane, do **not** pick XZ/YZ pancakes. Kill `ORBIT_SQUASH = 0.22` plane pick.
- Shared θ, period ~75–90s. Constant θ. Opposite slots (0 and π).
- **Pass:** fake Z from the same θ (`sin`): scale ~0.92–1.08, z-order between the inner two only. Same oval, not a second orbit.
- Radius: half their join separation, clamp **24–40pt** (one-to-two marble widths).

**Moons (3+)**

- Sticky inner pair = the two that formed the cluster (sorted ids at first join of 2). Additional marbles do **not** enter the inner oval.
- Outer ring: same frozen home, radius = inner + **36–48pt**, period slower (~120–180s), equally spaced among moons only.
- Moons get the same oval orientation and the same pass recipe (their own θ). Polar ease ~1.5s onto the outer ring. Inner pair does **not** re-seed when a moon joins.
- If an inner marble leaves (leave-dist or drag-away): recapture. If two remain, they become the new inner pair and freeze a new home from their live midpoint. If one remains, it sits. If 3+ remain, pick the two closest to each other as the new inner pair, rest moons.

**Keep-in / drag / still**

- If the **outermost** ring would leave `visibleFrame` (pad ≥ marble + 8), shrink radii toward the floor so the **whole figure** stays on-screen. Nudge **home** inward only if shrink isn’t enough. Never clamp each marble. Never target `(0,0)` / `visibleFrame.origin`.
- Isolates sit. Drag wins (~2s pin). Rejoin eases polar around the (unchanged) home — never a straight lerp.
- Reduce Motion stills + restores size (`orbit_restore`). Persist `worlds.json` x/y via `worlds_dirty` 10Hz. Do not persist home / θ / scale / inner-pair ids.

**Jerky**

- Today `tick` runs `anim_tick` on `n % 3 == 0` (~21 Hz) then `setFrameOrigin`. For this GO: when `marble_drift &&` any live marble cluster exists, run `drift_pass` **every** habitat tick (~16ms). Walker bob may stay on the 3-tick cadence. Do not introduce CADisplayLink. Do not add a physics integrator.

`habitat_stub` no-op. Settings tooltip: waltz / around each other — drop “slight drift” / “nearest planet.”

## Files

| File | Job |
|------|-----|
| `src-tauri/src/habitat_mac.rs` | `drift_pass` waltz + moons; drop hashed planes + home_drift; tick cadence |
| `src/lib/SettingsPage.svelte` | Tooltip only. Same key. |

## Forbidden

- Wells / poles / STARS / Talk / MCP / Jacob
- n-body, springs, velocity, Kepler, mass, screensaver, cursor-chase, default-on
- Equal slots for 3+ on the inner oval
- Hashed XY/XZ/YZ pancakes, `ORBIT_SQUASH` 0.22 as a plane
- Home creep, live centroid, nearest-expanded host
- Per-marble screen clamp
- `tauri dev`, STAMP_*

## Dogfood

1. Tray **Quit** → only `~/Applications/Orbitty.app` → AX re-grant
2. Settings → Appearance → **Marbles drift** on
3. Two nearby: slow mild oval around where they met; pass in front/behind; home does not walk
4. Collapse a third nearby: pair keeps waltzing; third takes a slower outer oval
5. Isolates sit; drag wins; expanded does not steal
6. Motion should read smoother than 1d (no pancake, no 21 Hz hop)
