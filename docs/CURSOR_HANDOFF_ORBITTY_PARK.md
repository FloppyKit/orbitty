# CURSOR HANDOFF — GO-ORBITTY-PARK

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_PARK_LOCK_v0.md` first.

## Files

- `src-tauri/src/panes.rs` — hook the **end** of `show_shared_pane` (~253)
- `src-tauri/src/drawers.rs` — only if clamp/place must be `pub(crate)`
- `src/lib/CompanionPage.svelte` — **none** if `set_companion_expanded` + `state:changed` already flips layout (it does)

## Shared-pane fact

Board and STARS are **one** window: label `"kanban"`. There is no `"stars"` spec. Park on `"kanban"` always. Do not spawn a second overlay.

Do not name the helper `park_warm_drawer` (that parks hidden chrome). Do not call `follow_docked_panes`.

## Do

1. Add `park_companion_on(app)` and call it after `show_labeled(app, "kanban")` succeeds inside `show_shared_pane`. One hook covers dest-strip, rail stack, Smallest-todo, later tray Board.
2. If `state.companion.expanded`, `set_companion_expanded(app.clone(), false)` **then** move. Collapse is a layout flag. **Do not `set_size`.**
3. Move the **existing** companion window to the `kanban` pane's lower-right, **8px inset**, current outer size, clamp on-display (`clamp_drawer_rect` / `place_logical_rect`). Do not cover kanban × (top-left). Skip if `kanban` has no outer_position. Always re-park on open.
4. If companion hidden: `show_companion` then park.
5. No leash. Later Board moves do not drag companion. Settings / Planets / ⌘⇧O / Sky do not park.
6. No new persist key. No `hitchedTo`.
7. `./scripts/install-stable.sh`; stop.

## Do not

CLEAR / shaped mouse. Embed Board. Second spirit. `set_size`. `tauri dev`. Stamp. Fold PLANETS-POP.
