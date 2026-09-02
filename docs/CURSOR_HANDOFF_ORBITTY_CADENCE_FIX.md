# Cursor Ultra — GO-ORBITTY-CADENCE-FIX

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**SoT:** `~/goose artifacts/ORBITTY_RUNTIME_AND_WORLD.md` §3, §8 · original `docs/CURSOR_HANDOFF_ORBITTY_CADENCE.md`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**Gate:** Matt names `GO-ORBITTY-CADENCE-FIX`
**Do not parallel with VOID** (same habitat). Do not start SLEEP / TASK / GATE.

## Why
Matt dogfooded CADENCE 2026-09-01:

1. Create todo → drag to Doing → **long beach ball** (it did land).
2. Claim / add a window → bodies **pile at the front-center** of the disc.
3. **Ghost** bodies; dragging one beach-balls again.
4. Card → Done → walker **does not leave**. It walks to the **middle and idles**.

EVENT-LOG is fine (`board-move` + one `walker-pose work→walkoff` did write). Do not reopen the log crate.

## Root causes (already read — do not re-litigate)

### A. Open windows are still dots
`habitat_mac/mod.rs` `do_sync` still partitions `minimized` → walkers, `open` → dots. CADENCE only poses **walkers**. A claimed **open** window never gets a station. Dots live on the inner ring (`radius * 0.55`) which reads as **front-center**. Workbench is also `(0.0, 0.45)` — south / near rim — so even a Doing *walker* looks like “front center.”

v1.1: **walker = claimed window**, not “minimized claim.” That partition is now wrong.

### B. Beach ball = main-thread stall
`sync()` does `run_on_main_thread` + `recv_timeout(2s)`. CADENCE put `cardStatus` on the habitat signature, so **every board drag** now full-resyncs. Inside `do_sync` (habitat lock, often main): `make_being` rebuild, `append_best` **disk IO**, `emit_walked_off` storms for Done dots. Lists `mutate` → `save_state` → `state:changed` → rail `syncHabitat` is the todo→Doing beach ball. Ghost drag repeats it.

### C. Done walks through the origin, then often becomes Roam
WalkOff target is `DONE_RIM (0.0, -0.55)` — from the workbench `(0.0, 0.45)` that is a straight line **through disc center**. If `card_status` drops to `""` mid-walk (unlinked window card, doneCard not picked, unclaim race), pose becomes **Roam** and they idle where they are: the middle. Walk-off emit is also fired on **every sync** for Done entries that are dots, which races the visible walk.

## Terminal goal
Board column is visible on the disc without stalling the UI.

| Board | Body | Pose / place |
|---|---|---|
| claimed, Todo | **walker** (open or minimized) | walk to bench, stand |
| claimed, Doing | **walker** | walk to workbench, work |
| claimed, Needed open/later | **walker** | snap to gate, sit |
| claimed, Done | **walker** | walk **off the rim**, then existing unclaim |
| claimed, no card | **walker** | stand in place (no amble while anyone works/sits) |
| parked / age `park` / marble | dots / bare | unchanged |
| sleeper | walker, still, dim | unchanged — cadence does not override unless Done |

Unclaimed windows are not on the planet. Loose todos (no `windowId`) never spawn or retarget a walker.

## Do

### 1. Claimed → walker (the visible bug)
In `do_sync`, **claimed entries walk** unless `age == "park"` (and not asleep) or the planet is a marble. Open-vs-minimized no longer decides walker vs dot.

Dots = parked only (plus whatever the marble path already empties). Do not invent a fifth occupancy.

Workbench must not read as “everyone standing on the nose.” Keep the four anchors, but **fan them** (already hashed in `station_target`) and move Doing off dead-center if it still stacks on `(0, 0.45)` — e.g. workbench `(0.0, 0.38)` is fine; do **not** add buildings.

### 2. Kill the beach ball
- If `sync()` is already on the main thread, run `do_sync` **inline**. Never `recv_timeout` on main waiting for main.
- `append_best` / any jsonl write: **not** while holding the habitat lock and **not** on the AppKit thread. Pose events: snapshot the transitions, append after the lock drops (or `append_best` from a background queue). A missed pose line is better than a beach ball.
- `emit_walked_off`: once per walker per walk-off (`off_emitted`), not on every sync for every Done dot. Re-delivery stays for a walker actually past `DONE_HIDE_Y`.
- Pose-only snapshot changes must **not** rebuild `Being`s or re-decode icons. Update `pose` in place; roam state stays.

Board drag of a **loose todo** (no window, no claim change) must not stall. If the habitat signature would change only because of an unlinked card, skip `sync_habitat`.

### 3. Done actually leaves
- WalkOff target is **rim then off** — never `(0,0)`, never Roam mid-walk. Keep `card_status: "done"` on the snapshot until the rail unclaim lands (already the `doneCard` fallback — make sure it still wins after this partition change).
- Do not emit `habitat:walked-off` until the sprite is past the hide line **or** Reduce Motion snap. No “fast-forward unclaim” for an on-disc walker.
- After unclaim, the being is gone. No ghost, not hittable (`input.rs` already skips hidden — include WalkOff-past-hide and dead window ids).
- Ghosts already on disk: a claim whose window is gone and not a sleeper → existing sleeper path or unclaim. Do not leave a roaming body with no window and no sleeper.

### 4. Stations stay the four columns
No CPU sniffing. Sleepers still. Ambient amble still dies while anyone works/sits.

## Files
- `src-tauri/src/habitat_mac/mod.rs` — partition, sync/recv, pose-in-place, emit
- `src-tauri/src/habitat_mac/tick.rs` — WalkOff never retargets to origin; no Roam flicker
- `src-tauri/src/habitat_mac/input.rs` — ghosts not hittable
- `src/lib/RailPage.svelte` — snapshot rows = claimed walkers; skip habitat invoke when only an unlinked card moved; walked-off listener stays idempotent
- `src-tauri/src/event_log.rs` — do not change the envelope; only call sites / thread

## Do not
VOID seed · sleep presets · task snapshots · gate/caps · `tauri dev` · stamp · rusqlite · fifth pose · Kenney buildings as stations · toasts

## Verify
`./scripts/install-stable.sh`. Quit Orbitty, open `~/Applications/Orbitty.app`.

1. Claim an **open** window (do not minimize). Walker appears and walks to **Doing / workbench**, not a dot on the nose.
2. Drag that card Todo → Doing → Needed → Done. Pose/station follow. **No beach ball.** Done walks off the rim and the claim drops. Nothing idles in the middle.
3. Add a loose todo, drag Todo → Doing. Card moves. Habitat does not stall. No extra walker.
4. Close a claimed window (sleeper). Still, dim, not a ghost you can drag into a hang.
5. `tail -5 ~/Library/Application\ Support/space.orbitty.mac/events.jsonl` — `board-move` / `walker-pose` still append; no flood of walked-off.

Stop when install-stable exits 0.
