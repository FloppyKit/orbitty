# CURSOR HANDOFF — GO-ORBITTY-HOST-SPLIT

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_HOST_SPLIT_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/CompanionPage.svelte` — well, dest rename/toggle, mount, Smallest-todo
- `src/lib/KanbanPage.svelte` — `embedded` + `onClose` + `onPopout`
- `src/lib/StarsPage.svelte` — embedded × → `onClose`; no pop-out
- `src-tauri/src/panes.rs` — **delete** `park_companion_on` call in `show_shared_pane`; host grow/restore (`1080×620`, stash idle rect)

## Do

1. Dest **Board** label → **Launchpad**. Click toggles the Launchpad well. Do **not** `invoke("open_kanban")` / `open_stars` for dests. Planets stays `bring_all_front`.
2. Hosted layout: collapsed chrome only. Left ~2/3 = `KanbanPage` or `StarsPage` (`embedded`). Right 1/3 = Head `.stage`. Below = existing two CC lines (truncated …) + agentrow + composer. Force `set_companion_expanded(false)` on enter. Do not mount `.transcript`.
3. Enter: hide `kanban` overlay, `set_size` companion **1080×620**, stash idle rect first. Dismiss / well × / dest-toggle-off: restore stash (else 520×780). Session-only hosted flag.
4. Launchpad pop-out (embedded banner only): `show_shared_pane("board")`, dismiss well, restore idle size. STARS has no pop-out. If kanban already visible and dest Launchpad is clicked, absorb into the well.
5. `openSmallestTodo`: enter Launchpad well and focus the card in the embedded page — do not open the overlay.
6. `./scripts/install-stable.sh`; stop.

## Do not

Call `park_companion_on`. Spawn `"stars"`. Pop-out STARS. Persist `hitchedTo`. Fold PLANETS-POP / BOARD-ACTIONS / CLEAR. `tauri dev`. Stamp.
