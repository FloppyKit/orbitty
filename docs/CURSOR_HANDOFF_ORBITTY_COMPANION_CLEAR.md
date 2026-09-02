# CURSOR HANDOFF — GO-ORBITTY-COMPANION-CLEAR

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_COMPANION_CLEAR_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src/lib/CompanionPage.svelte`
- `src-tauri/src/panes.rs`
- `src-tauri/src/coordinator.rs` (`set_companion_expanded`)
- `src-tauri/src/lib.rs` (register command)
- poll helper: new `companion_clear.rs` **or** piggyback `habitat_mac/tick.rs` / `drawers.rs`

## Do

1. Ghost = shown companion, not expanded, not hosted. `.companion.ghost { background: transparent }`. Slab (expanded or hosted) keeps today’s fill and never click-throughs.

2. Frontend reports window-local CSS-point rects for: spirit **image box** (not the empty `.stage`), CC rows, starstrip, agentrow, composer, ×, ⚙, note, planets popup. `invoke("set_companion_hits", { rects })`.

3. Native poll (~10Hz, no second DisplayLink): mouse over a hit rect → `set_ignore_cursor_events(false)`; else ghost → `true`. Expanded persist or `host_stash` Some → always `false`.

4. `set_companion_expanded(true)` and `set_companion_host(on)` force slab immediately. Show path must re-apply mode so `show_drawer`'s current `ignore=false` does not stick on a ghost.

5. Drag: stay mouse-on until the drag ends.

6. `./scripts/install-stable.sh` then STOP.

## Do not

`tauri dev`. Stamp. PNG sampling. Click-through on well / expanded / other panes. Walker menu. Action edit.
