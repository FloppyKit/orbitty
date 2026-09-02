# Cursor Ultra — GO-ORBITTY-CADENCE-PATH

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**Gate:** Matt named `GO-ORBITTY-CADENCE-PATH`
**Do not parallel VOID.** Do not start SLEEP / TASK / GATE. Do not reopen EVENT-LOG.

CADENCE-FIX (occupancy + main-thread recv) stays. This GO is **pathing only**.

## Dogfood (Matt 2026-09-01 21:01)

> Doing: they walk along the **front rim**.
> Done: they walk back and **idle toward the upper middle**. They never leave.

Log agrees: `doing→done` + `work→walkoff` for `878:130`, then **no unclaim**. Pose is WalkOff. They never crossed `DONE_HIDE_Y`.

## Why (already read — do not re-litigate)

View is flipped (`isFlipped`): disc **+y = screen DOWN = front / near**. Oblique `radius_y = 0.55 r`, so disc `|y|=1` is the **visual** rim.

| Const | Value | What Matt sees |
|---|---|---|
| `STATION_WORKBENCH` | `(0.0, 0.38)` | front / near rim |
| Work pose after arrival | walk cycle in place (`stepping = true`) | **walking along the front rim** |
| `DONE_RIM` | `(0.0, -0.55)` | 55% toward far side = **upper middle of the oval** |
| `DONE_HIDE_Y` | `-0.85` | never reached, so no emit / no unclaim |

`walkoff_target` already skirts `(0,0)`. It still **homes at `DONE_RIM y=-0.55`**, which is not an exit.

## Terminal goal

| Board | Motion |
|---|---|
| Doing | Walk **to** the workbench, then **work in place** — idle frame 0 (or a work cycle that is **not** the walk cycle). Not rim-strolling. |
| Done | Walk **off the far / top rim** (horizon, shrinking) and vanish. Then existing `habitat:walked-off` unclaim. Never idle on-disc. |

Todo bench (west) and Needed gate (east) stay. Sleepers stay still. No CPU sniffing.

## Do

### 1. Doing is a station, not a rim parade
- Move `STATION_WORKBENCH` **in** from the lip — about `(0.0, 0.22)` is fine (still south / near, clearly on the disc).
- Transit (`dist >= 0.025`) still uses the walk cycle.
- **Arrived Work:** `stepping = false`. Idle frame 0, bob 0 (same as Stand). Tiny work-bob is ok; the 4-step **walk** cycle is not.
- Do not re-park. Do not amble.

### 2. Done leaves the disc
Disc `|y| ≈ 1` is the visual rim. Current `-0.55` is the interior.

Suggested (tune if the oval still clips):

```
DONE_RIM     (0.0, -0.92)   // far / top rim, on the disc
DONE_OFF     (0.0, -1.40)   // past the ellipse
DONE_HIDE_Y  -1.08          // hidden + emit once past this
```

`walkoff_target`: first the far rim, then `DONE_OFF`. Waypoints stay on `|r|≈0.85` **sides**, never a y that reads as disc-middle (`|y| < 0.70` is not an exit). If current target is reached (`dist < 0.025`) and pose is still WalkOff, **immediately** retarget `DONE_OFF` — never idle.

Hide + `emit_walked_off` when `roam.y < DONE_HIDE_Y` (once, `off_emitted`). Rail unclaim stays as-is.

Sticky WalkOff (blank `card_status` → don’t Roam) stays.

### 3. Compass
Far / top of the **screen** = Done exit (walker shrinks into the horizon). Near / bottom = Doing workbench. Do not flip the CADENCE-FIX y signs again.

## Files
- `src-tauri/src/habitat_mac/mod.rs` — constants, `walkoff_target`, hide line
- `src-tauri/src/habitat_mac/tick.rs` — Work arrived ≠ walk cycle; WalkOff never idles
- Draw frame law in `mod.rs` (~2057): Work arrived uses idle frame, not walk frames

## Do not
VOID · sleep presets · task snapshots · gate/caps · `tauri dev` · stamp · occupancy rewrite · jsonl crate · toasts · Kenney buildings as stations

## Verify
`./scripts/install-stable.sh`. **Quit Orbitty and open `~/Applications/Orbitty.app`** (a new binary is not the running process).

1. Claim an open window. Doing: walker walks to a **south station on the disc** and stands/works. Not pacing the front lip.
2. Drag the card to Done. Walker walks **up / off the top rim** and disappears. Claim drops. Nothing idles in the upper middle.
3. `tail -5 ~/Library/Application\ Support/space.orbitty.mac/events.jsonl` — `work→walkoff` then an `unclaim` for that window.

Stop when install-stable exits 0.
