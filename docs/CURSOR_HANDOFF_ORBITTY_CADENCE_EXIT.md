# Cursor Ultra — GO-ORBITTY-CADENCE-EXIT

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**Gate:** Matt named `GO-ORBITTY-CADENCE-EXIT` (dogfood after PATH, 2026-09-01 21:23)
**Do not parallel VOID.** Occupancy / recv / sticky WalkOff stay.

## Dogfood

> Doing: they walk toward the **front lip** and **just stand**. No motion.
> Done: they go to the **upper end of the disc**, start walking, and **never leave the screen**.

PATH 21:19 binary is running (sock 21:20). Log: `doing→done` + `work→walkoff` for `70483:6731`, **no unclaim**.

Doing-idle is PATH working as designed. Do **not** restore the in-place walk cycle. Remaining bugs are destination + exit.

## Why (already read — do not re-litigate)

1. **Workbench still the front lip.** `STATION_WORKBENCH = (0.0, 0.22)` is south/near. Flipped view: +y = screen down = front. 0.22 still reads as “on the nose.”

2. **`walkoff_target` yoyos.** Tick retargets `DONE_OFF` when `dist < 0.025`, then the **next** tick `walkoff_target` sees `d_rim >= 0.05` and returns `DONE_RIM` / shoulder `(-0.45, -0.72)` again. Walker walks the far rim forever. `DONE_HIDE_Y = -1.08` is never crossed → no emit → no unclaim.

3. **Oval clip.** Draw path `bezierPathWithOvalInRect` clips **everything including walkers**. Disc `|y| > ~1` is not drawn. Even a correct `DONE_OFF (-1.40)` is invisible *and* still on-planet until hide, so they cannot “leave the screen” — they grind the clipped horizon.

## Terminal goal

| Board | Motion |
|---|---|
| Doing | Walk to a **south station well inside the disc** (not the lip). Arrive → idle frame 0. No walk-cycle, no rim pace. |
| Done | Walk **off the far / top rim onto the panel** (past the oval), shrink/vanish, `habitat:walked-off` once, rail unclaim. Never reverse. Never idle on-disc. |

## Do

### 1. Workbench off the lip
`STATION_WORKBENCH ≈ (0.0, 0.10)` (still south / near half, clearly interior). Fan via existing `station_target` hash. Todo west / Needed east stay.

Arrived Work stays idle frame 0. Do not bring back the walk cycle.

### 2. WalkOff is monotonic — kill `walkoff_target` as a homing loop
Once pose is WalkOff, **never** choose a target with `y` greater (more south / toward the workbench) than current `roam.y`.

Simplest correct law:

```
WalkOff:
  if roam.y < DONE_HIDE_Y: emit once; hide; continue
  tx, ty = DONE_OFF            // always, every tick
  step toward (tx, ty)
```

Optional hashed `tx = ±0.20` so two Done walkers don’t overlap — both `ty = DONE_OFF.y`. Delete the rim/shoulder ping-pong. `walkoff_target` may die or become “return DONE_OFF.”

Constants (tune only if the oval still clips the sprite before hide):

```
DONE_OFF     (0.0, -1.55)   // past the ellipse, onto the panel
DONE_HIDE_Y  -1.12          // emit once past the visual rim
```

`DONE_RIM` is unused if WalkOff always aims `DONE_OFF`. Don’t keep a rim waypoint that can pull `y` back up.

If `dist < 0.025` and still WalkOff and `y > DONE_HIDE_Y`, **snap** `roam.y = DONE_HIDE_Y - 0.01` and emit — never freeze.

### 3. Draw WalkOff **outside** the oval clip
Walkers in `Pose::WalkOff` (and only those) paint **after** the disc clip `restoreGraphicsState`. Decor / dots / Work / Stand / Sit stay clipped. The leave must be visible: sprite walks off the top of the planet onto the square panel, then hide.

Do not draw off-disc Work/Stand (that would look like falling off while Doing).

### 4. Compass
Far / top of the **screen** = Done exit. Near / bottom = Doing. Do not flip CADENCE-FIX y signs.

## Files
- `src-tauri/src/habitat_mac/mod.rs` — workbench const, WalkOff draw after clip, hide
- `src-tauri/src/habitat_mac/tick.rs` — monotonic WalkOff; no yoyo
- `walkoff_target` — delete or make it `DONE_OFF` only

## Do not
VOID · sleep · task · gate · `tauri dev` · stamp · occupancy rewrite · jsonl crate · restore Work walk-cycle · toasts

## Verify
`./scripts/install-stable.sh`. **Quit Orbitty and open `~/Applications/Orbitty.app`.**

1. Claim → Doing: walker walks to an **interior south station** and stands still. Not on the front lip, not pacing.
2. Card → Done: walker walks **off the top of the disc onto the panel** and disappears. Claim drops within a couple seconds.
3. `tail -8 ~/Library/Application\ Support/space.orbitty.mac/events.jsonl` — `work→walkoff` then `unclaim` for that window.

Stop when install-stable exits 0.
