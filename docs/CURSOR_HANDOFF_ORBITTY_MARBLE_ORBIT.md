# Cursor Ultra — GO-ORBITTY-MARBLE-ORBIT

**Model:** `kimi-k3-max` · **one writer**
**Gate:** Matt **GO-ORBITTY-MARBLE-ORBIT** via Pixel Artist — **issued 2026-08-28**. Code this. Part 1b of 2. No wells.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_SOLAR_NEST_TALK_v0.md` (Marble fidget) + this handoff
**Part:** **1b of 2.** Revises part 1 motion. Same `marbleDrift` toggle. **No wells.** Part 2 remains `GO-ORBITTY-GRAVITY-POLES`.

Matt 2026-08-28 eyes-on of `GO-ORBITTY-MARBLE-DRIFT`: no physics, feels slightly magnetized, animation jerky. He wants clustered marbles to **orbit each other** by default, able to pass on **X, Y, or Z** so the desk reads depth. Z is fake (scale + z-order). Still kinematic. Not n-body.

## Why this is a new GO

Part 1 shipped **self-ellipses**. Each nearby marble walks its own 12–28pt oval around the spot it sat. Neighbors within 220pt only **unlock** that fidget. Isolates freeze. That is a magnet switch, not an orbit.

Handoff for part 1 said *cluster orbits its centroid*. Kimi did not ship that. Do **not** stamp part 1 as the desired motion. Rewrite `drift_pass` (and `drift_params` if it is now unused). Keep the toggle, pin, clamp, persist, Settings row.

## Law

1. **Marble only.** `Planet.is_marble == true`. Expanded never translates.
2. **Same toggle.** `marble_drift` default **false**. No second switch. Settings → Appearance → **Marbles drift**.
3. **Drag wins.** `h.drag` set → no marble translates. Drop pins ~2s, then may rejoin. Clicks must still hit the disc.
4. **Reduce Motion still.** `h.reduce_motion` early-return already stills `anim_tick`. Restore default marble size / z-order if you scaled.
5. **Move NSPanels.** No canvas, no physics integrator, no velocities, no mass.
6. **No wells / poles / nest / n-body / cursor chase / screensaver.**
7. **No STARS, Talk, MCP, ctl, Jacob, Spark.**
8. Dogfood **only** `~/Applications/Orbitty.app` after tray Quit.

## Motion (replace `drift_pass`)

Same screen. **Connected component**, not “each marble vs nearest.”

**Hysteresis (kills the magnet jerk):**
- Join cluster if center-to-center ≤ **220pt**
- Leave only if ≥ **280pt**
- Do not clear orbit state on a one-frame flicker

**Membership:**
- Component of 1 → sit. Restore scale 1. Clear per-cluster orbit state for that id.
- Component of 2 → **pair**: shared midpoint, 180° apart
- Component of 3+ → shared **centroid** (mean of current centers), equally spaced on the ring

**Shared clock.** One `θ` per cluster from `h.bob_phase / period`. Period **60–90s**, hashed from **sorted** member ids (stable while the set is stable). Do **not** give each marble its own period/phase ellipse around a rest anchor.

**Radius.** They must actually go **around each other**, not vibrate in place.
- `R = clamp(max(MARBLE_RADIUS * 2.2, 0.45 * mean_dist_to_centroid), 28, 90)`
- Pair: start from half their separation, then clamp
- On-screen clamp (visibleFrame, pad ≥ `MARBLE_RADIUS + 8`). Never hop displays

**Plane (fake 3-axis).** Hash of sorted member ids → `{XY, XZ, YZ}`. Cluster keeps that plane until membership changes.

| Plane | Screen x,y | Depth |
|-------|------------|--------|
| **XY** | `c + R(cos θ, sin θ)` | scale = 1, z-order unchanged |
| **XZ** | `c + (R cos θ, k R sin θ)` with `k ≈ 0.22` | `z = sin θ` → scale + z-order. They **pass in front/behind** when screen positions overlap |
| **YZ** | `c + (k R sin θ, R cos θ)` | same depth channel |

**Fake Z (XZ / YZ only):**
- Scale the marble panel **0.88–1.12** from `z` (nearer = larger). Do **not** persist scale. Drift off / isolate / Reduce Motion → `MARBLE_RADIUS` size.
- Among clustered marbles only: nearer (`z` higher) `orderFrontRegardless` after the farther ones. Do not fight expanded planets or other apps.
- Click must still hit. Do not scale below 0.88.

**No 2D collision.** Passing on Z **requires** overlapping screen x,y. Do not push them apart.

**Join / leave:** lerp from the live frame onto the parametric orbit over **~0.8s** so membership changes do not teleport. After that, set origin from the parametric point (smooth `θ` is enough; do not add a physics step).

**Isolates sit.** Expanded sit. Pinned (~2s after drop) sit, then re-seed into whatever cluster they are in.

**Persist:** live `x`/`y` via existing `worlds_dirty` 10Hz path. Do not persist scale, plane, or `θ`.

## Files (expected)

| File | Change |
|------|--------|
| `src-tauri/src/habitat_mac.rs` | Rewrite `drift_pass`. Drop per-id self-ellipses / rest-anchor gait as the motion. Keep toggle gate, pin, screen clamp, `worlds_dirty`. Cluster plane + fake Z here. |
| `src-tauri/src/habitat_stub.rs` | Untouched. |
| Settings / `state.rs` / `habitat.rs` / `+page.svelte` | **No new keys.** Toggle stays `marbleDrift`. Copy may say they slowly orbit each other (depth on some clusters). |

## Forbidden

- Gravity wells / poles / STARS chrome
- n-body, velocity, mass, springs, “magnetism” as attraction
- Keeping the part 1 self-ellipse as the ship motion
- Default-on
- Ignoring Reduce Motion or drag
- Scaling so clicks miss
- Talk / Speech / Handy / MCP / ctl / Jacob
- `tauri dev` / `target/` dogfood
- `STAMP_*`

## Verify (blessed app)

1. Toggle off: marbles stay put. Size 18pt.
2. Toggle on, two nearby, Reduce Motion off: they **circle each other** around a midpoint — not two tiny ovals on their own spots.
3. Three nearby: one ring around the group centroid.
4. Some clusters pass in front/behind (scale + z-order). Some stay flat XY. Hash is stable for a given set.
5. Lone marble sits. Expanded does not translate.
6. Drag freezes all; drop pins ~2s; click still hits.
7. Reduce Motion: still, default size.
8. No wells in this GO. `orbittyctl status` still AX trusted.

Acceptance: `docs/ACCEPTANCE-ORBITTY-MARBLE-ORBIT.md`

When done: `./scripts/install-stable.sh`. Summarize files + dogfood. Do **not** write `STAMP_*`. AX re-grant expected.
