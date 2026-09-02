# CURSOR HANDOFF — GO-ORBITTY-LP-NEEDED

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_LP_NEEDED_LOCK_v0.md` first. Parent lock `ORBITTY_LAUNCHPAD_SOT_LOCK_v0.md`.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src-tauri/src/state.rs` — additive fields on `KanbanCard`
- `src/lib/KanbanPage.svelte` — type, filter Needed out of quest views, strip, sidebar + Needed
- `src/lib/RailPage.svelte` — **one-line guard only** if `todosFor` / open-todo counts include Needed; no restyle
- `src/lib/CompanionPage.svelte` — **none**

## Do

1. `KanbanCard.kind` may be `"needed"`. Additive: `neededState?`, `neededWhy?`, `neededAnswer?`. Rust serde default skip if none. Old saves load.
2. Filter `kind === "needed"` out of Todo · Doing · Done, List rows, Today/Done/Inbox/project quest lists, Next chip, points.
3. Strip: full width under the banner, above `.k-body`. Only if at least one Needed is `open` or `later` (later with past `dueAt` counts as open). Quiet. Title + optional why. Answer (inline) / Skip / Later.
4. Sidebar `+ Needed` under Actions always — first card can be born when strip is hidden. Mints `kind: "needed"`, `neededState: "open"`, dummy `status: "todo"`, `projectId` = selected planet or `""`, id `` `n-…` ``. No points door.
5. Answer / Skip / Later via `mutate()`. Never `completeCard`.
6. `./scripts/install-stable.sh`; stop.

## Do not

Fourth column. Dest chrome. Agents well. Actor field. Talk. Auto-mint. `tauri dev`. Stamp. Fold #2–#4. Investigate PLANETS-POP beachball.
