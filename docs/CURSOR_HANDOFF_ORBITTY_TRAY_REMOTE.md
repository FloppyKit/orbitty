# CURSOR HANDOFF — GO-ORBITTY-TRAY-REMOTE

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_TRAY_REMOTE_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

## Files

- `src-tauri/src/lib.rs` — tray builder, left-click, `RunEvent::Reopen`
- `src/lib/CompanionPage.svelte` — one `orbitty:enter-host` listen → existing `enterHost`

## Trap

Today's **Lists** item calls `panes::show_labeled(app, "kanban")` — overlay. After HOST-SPLIT, dests never `open_kanban` / `show_shared_pane`. PARK is deleted. Tray Launchpad/STARS must `show_companion` + emit `orbitty:enter-host` + `set_companion_host` so the **well** opens. Do not copy the old Board/`show_shared_pane` lock.

`hosted` is page `$state`. Native resize alone does not show Lists.

## Do

1. `show_menu_on_left_click(false)`. Left Click Up → `panes::show_companion` + `emit_to("companion", "orbitty:companion-summon", ())`. **Not** `show_rail`.
2. Right-click menu, this order: Show Companion · Launchpad · STARS · Settings · Restore All Stowed Windows · Quit Orbitty.
3. Drop Show Orbitty and Lists. Label **Launchpad**, not Board.
   - Show Companion = `show_companion` + summon emit
   - Launchpad = `show_companion` + `set_companion_host("launchpad")` + `emit_to("companion", "orbitty:enter-host", "launchpad")`
   - STARS = same with `"stars"`
   - Settings = `open_settings_window` / `show_labeled("settings")`
   - Restore / Quit unchanged
4. Dock Reopen → `show_companion` + summon emit.
5. CompanionPage: `listen<string>("orbitty:enter-host", e => { if (e.payload === "launchpad" || e.payload === "stars") void enterHost(e.payload) })`. Not `toggleDest`. Unlisten on destroy next to summon.
6. Light/dark PNG swap unchanged. No Inspector item. No badge.
7. `./scripts/install-stable.sh`; stop.

## Do not

Inspector item. Badge. NSMenu of windows. `show_labeled("kanban")`. `show_shared_pane`. `open_kanban` / `open_stars` from tray. Menu label Board. Fold INSPECTOR / HOST-CC / action-edit. `tauri dev`. Stamp.
