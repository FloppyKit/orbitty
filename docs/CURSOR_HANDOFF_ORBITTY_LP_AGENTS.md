# CURSOR HANDOFF — GO-ORBITTY-LP-AGENTS

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_LP_AGENTS_LOCK_v0.md` first. **Illegal until LP-MINE dogfood.**

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/KanbanPage.svelte` — declare `agentShortcuts`, well, launch, mutate
- `src/lib/RailPage.svelte` — **do not steal**; copy `launch_agent` invoke shape from ~1192
- `src-tauri/src/state.rs` — **none** (struct already has `project_id`)
- Companion — **none**

## Strip trap

Kanban `mutate()` is load → mutate → save the **whole** object. Declare `agentShortcuts?: AgentShortcut[]`. `loadState` it. `s.agentShortcuts ??= []` before writes. **Never omit the key** if it was present.

Shape (copy rail): `{ id, name, harness, profile, model?, mdPaths: string[], projectId?: string | null }`.

## Do

1. Well on `.k-side`, **below Actions, above Today**. Heading **Agents**. List and Board, embedded and popped.
2. Rows: `projectId === selected` when selected is a planet id. Inbox = unassigned (`!projectId`). Today/Done = heading + hint “Select a planet”, no global dump.
3. Click → `invoke("launch_agent", …)` same args as rail. Note ok/err in the well.
4. `+` add; × delete; id format copy rail; `projectId` = selected planet or null.
5. `./scripts/install-stable.sh`; stop.

## Do not

Agents dest. Replace STARS. Soul picker. Walkers. Wipe `agentShortcuts`. Talk. Stamp. `tauri dev`. Steal rail lane UI.
