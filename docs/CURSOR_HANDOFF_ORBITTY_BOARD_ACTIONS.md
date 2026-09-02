# CURSOR HANDOFF — GO-ORBITTY-BOARD-ACTIONS

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_BOARD_ACTIONS_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/KanbanPage.svelte` — Actions well + persist `actionShortcuts` + Next chip
- `src/lib/RailPage.svelte` — **do not** steal; lane UI unchanged
- `src/lib/CompanionPage.svelte` — **none**

Copy rail's existing CRUD in `RailPage.svelte` (`addAction` / `deleteAction` / `runAction`): shape `{ id, name, command, cwd? }`, id `` `a-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}` ``, `invoke("run_action", { command, cwd })`, note `name: done` / `name: exit N`.

## Strip trap

Kanban `mutate()` is load → mutate → `save_state` the **whole** object. `PersistedState` in KanbanPage **does not declare** `actionShortcuts` (count 0 today). Add the field, `loadState` it, `s.actionShortcuts ??= []` before writes. **Do not omit the key** on save or you wipe the rail array.

## Do

1. Well at **top of `.k-side`** (visible List and Board, embedded Launchpad well **and** popped window). Heading Actions. Names from `actionShortcuts`. Click → `run_action`. Note ok/err in the well.
2. `+` add name/command/cwd. × delete. Same load→mutate→save. Round-trip `actionShortcuts`.
3. Next chip: smallest open todo (same rule as companion `smallestTodo`: first `doing` card, first undone subtask else title) → existing `onFocusCard` in this page. Hide if none.
4. No `List | Board | Actions` tab. Rail Actions **lane UI** untouched. Do not add a companion dest.
5. `./scripts/install-stable.sh`; stop.

## Do not

Companion dest. Agents tab. Wipe macros on a card complete. Fold ASK-ACTION / HOST-CC. Investigate PLANETS-POP beachball. `tauri dev`. Stamp.
