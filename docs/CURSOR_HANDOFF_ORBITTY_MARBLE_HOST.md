# Cursor Ultra — GO-ORBITTY-MARBLE-HOST

**Model:** `kimi-k3-max` · **one writer**
**Gate:** Matt **GO-ORBITTY-MARBLE-HOST** via Pixel Artist — **issued 2026-08-28**. Code this. Part 1c of 2. No wells.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_SOLAR_NEST_TALK_v0.md` (Marble fidget) + this handoff
**Part:** **1c of 2.** Revises 1b motion. Same `marbleDrift` toggle. **No wells.** Part 2 remains `GO-ORBITTY-GRAVITY-POLES`.

Matt 2026-08-28 eyes-on of `GO-ORBITTY-MARBLE-ORBIT`: motion is orbital but feels like **a pair slowly walking the screen**, then **yanked down**. He wants gravity to **prioritize the nearest planet** (the sitting expanded disc). Other bodies may tug slowly. Every tug is **orbital**, never a straight-line pull.

## Why this is a new GO

1b ships pair/centroid rings whose **center is the live average of the marbles**. That center walks, so the pair walks. Screen-edge clamp + 0.8s lerp then yanks them. That is not “orbit the nearest celestial object.”

Do **not** stamp 1b. Rewrite `drift_pass` host selection. Keep toggle, pin, fake Z, hysteresis-on-membership, persist x/y, Settings row.

## Law

1. **Marble only translates.** `Planet.is_marble == true`. Expanded discs **sit**. They are hosts, not walkers.
2. **Same toggle.** `marble_drift` default **false**. No second switch. Settings → Appearance → **Marbles drift**.
3. **Drag wins.** `h.drag` set → no marble translates. Drop pins ~2s, then may rejoin. Clicks still hit the disc.
4. **Reduce Motion stills.** Restore `marble_scale` 1.0.
5. **Kinematic.** Move existing NSPanels via `anim_tick`. No mass, velocity, n-body integrator.
6. **Orbital only.** A marble’s target is always a point on a ring (or slow radius ease onto that ring). **Never** lerp the marble straight at a host or neighbor. That was the yank.
7. **No wells / poles / nest / cursor chase / screensaver.**
8. **No STARS, Talk, MCP, ctl, Jacob, Spark.**
9. Dogfood **only** `~/Applications/Orbitty.app` after tray Quit.

## Host pick (the whole GO)

On each screen, independently:

1. **Host** = nearest **expanded** planet (`is_marble == false`) on that same screen, measured center-to-center from the marble.
2. **Hysteresis:** once a marble has a host, keep it until another expanded on that screen is nearer by **≥ 80pt**. No flicker between two discs.
3. **Several marbles, one host:** they share that host as ring center, equally spaced (same slot math as 1b’s 3+). One θ per host-group, hashed 90–150s period (slower than 1b’s 60–90 — walking-pair felt busy).
4. **No expanded on that screen:** fall back to **1b** (pair midpoint / marble centroid, 220/280 hysteresis). Do not leave a desk of only-marbles dead.
5. **Wells do not exist in this GO.** When part 2 lands, wells become another host type and win over expanded (user placed them). Do not implement that here.

Isolates: a lone marble **with** a host still orbits that host (that is the point). A lone marble **with no host** sits.

## Ring

- Center = **host’s live panel center**. Expanded never translates, so the center does not walk. That kills the pair-walk.
- Radius = `host.effective_radius + MARBLE_RADIUS + 36pt`, clamped **80–200pt**. Must **clear the disc** — marbles do not travel through the planet.
- If the ring would go off-screen: **shrink radius** down to clamp min (still ≥ host radius + marble + 12). **Do not shift the host center.** Do not clamp each marble independently in a way that flattens the bottom of the orbit into a “pulled down” slide.
- Join / host-change: lerp onto the new ring over **~1.5s** smoothstep. During that lerp the **center is already the host**, radius eases; the marble is not flown at the planet.
- Pin ~2s after drop, then rejoin.
- Fake Z: keep 1b hashed XY / XZ / YZ from sorted member ids of the **host-group**, scale 0.88–1.12 + z-order among those marbles only. Panel size stays 96pt; `marble_scale` is draw + hit-test only.

## Weak other bodies (v0)

Other expanded planets on the same screen do **not** add a second center. They only steal the marble when hysteresis says they are nearer.

Other marbles: equal spacing on the shared host ring. No extra pairwise pull. No “slow walk toward neighbor.”

If you are tempted to add a second force, it must be **θ precession** (the whole ring turns a hair), never a translation of the center toward anything.

## Persist

Live x/y via existing `worlds_dirty` 10Hz. Do **not** persist scale / plane / θ / host id. Host is recomputed each tick.

`habitat_stub` stays no-op.

## Files

| File | Job |
|------|-----|
| `src-tauri/src/habitat_mac.rs` | Rewrite host pick + ring in `drift_pass`. Expanded sit. Radius-shrink clamp. |
| `src/lib/SettingsPage.svelte` | Tooltip only, if needed: “orbit the nearest open planet.” Same key. |
| `habitat_stub.rs` / `state.rs` / ctl / MCP | **untouched** |

## Forbidden

- Wells, poles, Drop well, `worlds.json` `wells[]`
- Mass / velocity / n-body
- Direct (radial) pull toward a body as the motion
- Letting the ring center be the marble centroid when a host exists
- Default-on, STARS, Talk, MCP, ctl, Jacob, `tauri dev`, `STAMP_*`

## When done

`./scripts/install-stable.sh`. Summarize files. Do **not** write `STAMP_*`.
