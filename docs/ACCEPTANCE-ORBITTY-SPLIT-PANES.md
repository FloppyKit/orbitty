# ACCEPTANCE — GO-ORBITTY-SPLIT-PANES

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

## Must

- `src-tauri/src/panes.rs` exists. `lib.rs` is tray + `run()` + `show_rail` + one-line `open_*` + AX/chars/packs/coordinator handlers.
- Board / STARS / Companion / Settings / Cloud still open. No 404. No native overlay titles.
- STARS shares the 780×520 kanban overlay. `?mode=` still banned. Leftover `stars` window, if any, hides never destroys.
- Grab banner, drag, hide, reopen = last mostly-on else primary-center. 12px corners + shadow.
- Rail click/double-click does not vanish. Dock / tray Show restore the rail.
- Boombox click-stay + fader unchanged. Wordmark ochre i-dot unchanged. Walker Assign-to-planet unchanged.

## Must not

Spawn `stars`. Restore leash. Hide after show. Hide on bad nav. Trailing-edge / maximize companion. Behavior change. Restyle. `tauri dev`. Stamp from the writer.
