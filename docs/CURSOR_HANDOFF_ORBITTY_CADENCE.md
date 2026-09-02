# Cursor Ultra — GO-ORBITTY-CADENCE

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**SoT:** `~/goose artifacts/ORBITTY_RUNTIME_AND_WORLD.md` §3, §8
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**Gate:** Matt names `GO-ORBITTY-CADENCE`
**Do not parallel with GO-ORBITTY-VOID** (same habitat sync).

## Terminal goal
Walkers narrate the **board**, a beat behind. Four poses, four stations = four columns. No CPU sniffing. Sleepers stay still (already shipped). Ambient random roam dies the moment any walker on that planet is working or sitting.

## Mapping (one table — put it in **one** place)

| Board | Station (disc-local target) | Pose |
|---|---|---|
| Todo | bench | stand |
| Doing | workbench | work (walk there, then work) |
| Needed (open/later) | gate | sit — **skip the walk queue**, snap/short walk |
| Done | rim → off | walk off, then existing Done eviction (sprite goes) |
| no card | stand in place | stand |
| asleep | wherever laid out | sleeper (existing). Do not cadence-override |

Window-linked `kind=window` cards win for that `window_id`. Needed that names the same window/project sits that walker. If both Doing and Needed, **Needed wins**.

## Files
- `src-tauri/src/habitat_mac/mod.rs` — `Being.pose`, station anchors, sync reads cards
- `src-tauri/src/habitat_mac/tick.rs` — roam toward station; no random park while work/sit present
- `src-tauri/src/habitat.rs` — `HabitatEntry` additive pose if the snapshot crosses the FFI; else keep pose native-only
- Native already receives kanban via rail snapshot / `syncHabitat`. **Use that.** If cards are not in the snapshot, add `status` + `kind` + `neededState` per window id to the existing snapshot struct — do not open a second channel.
- Optional: `invoke("log_event")` from native on pose change (at most one per walker per transition). If EVENT-LOG is not shipped, wrap so a missing command cannot break habitat.

## Stations
Four **anchors on the disc**, not Kenney buildings. Disc-local, radius ~0.45–0.55:

| Station | Approx |
|---|---|
| bench (Todo) | west, `(-0.45, 0.05)` |
| workbench (Doing) | south, `(0.0, -0.45)` |
| gate (Needed) | east, `(0.45, 0.05)` |
| done-rim | north, `(0.0, 0.55)` then continue off (`y > 0.85` → hide until Done eviction)

Constants, named, one file. Do not read decor building positions.

## Poses (reuse frames — no new art)
- **stand** — idle frame 0, bob 0 (already paused/asleep look without the sleeper dim)
- **work** — existing walk cycle in place (bob + walk frames, `tx≈x`)
- **sit** — idle frame 0, no bob, no roam (Needed). Distinct from sleeper: full brightness, not dim
- **walk** — existing roam toward target

Cap = these four. No fifth.

## Roam law
- Target = station of the card, not a random `rad`.
- Arrival `dist < 0.025` → pose stand or work; do **not** pick a new random tx.
- If any walker on that planet is `work` or `sit`, **all** random amble is off (even Todo standers).
- Needed: retarget to gate immediately; skip remaining amble steps.
- Done: walk to rim then off. Eviction still comes from existing Done/unclaim — cadence must not delete the claim.
- Sleepers: `continue` as today. Never retarget.

## Do
1. Snapshot carries enough board truth for each claimed window.
2. Tick seeks stations. Kill random park while work/sit live.
3. Sit ≠ sleeper (brightness).
4. `./scripts/install-stable.sh`. Stop.

## Do not
CPU / PTY / `top` · Kenney houses as stations · new sprite sheets · PSX agent bodies · The Void · sleep presets · vanish-on-close (that's sleepers) · Complete-on-walk-off · `tauri dev` · stamp

## Verify
Claim a window, card in Todo → walker stands west. Drag to Doing → walks south and works. Mint Needed for it → sits east now, not after a scenic route. Done → walks north off; rail Done still the eviction. Close window → sleeper dim, not sit.
