# CURSOR HANDOFF — GO-ORBITTY-PANE-WARM

Sole writer. Pixel does not edit product files.

- SoT: `~/.buzz/PLANS/ORBITTY_PANE_WARM_LOCK_v0.md`
- Acceptance: `docs/ACCEPTANCE-ORBITTY-PANE-WARM.md`

PANE-GLASS dogfood green. First rail click of Companion / Board / STARS / Settings beach-balls, then it's fine. Cause: lazy `ensure_window` on the click + SPA mount-check `reload()` on a hydrating view (`drawer_mount_ok` is `main_thread_sync`).

SPLIT-DRAWERS may have moved `show_drawer` / `schedule_drawer_mount_check` into `drawers.rs`. Edit the file that owns the symbol. `ensure_window` may still be in `lib.rs`.

## Do

A. `run().setup`, after rail wiring, `ensure_window` for `companion`, `kanban`, `settings`. Stay hidden. Stay click-through. Do not `show()`. Do not pin companion until shown. Not `stars`. Not `sky`.

B. `schedule_drawer_mount_check`: if the window was created < ~3s ago, first AX read only — no `reload()`, no navigate. Warm pane alive >3s with empty WebArea still gets the existing one reload + one navigate. Never hide. Never loop.

C. Keep MATCH / SPA / HOLD / GLASS / pin-while-shown / COVER / STARS-shares-kanban / `?mode=` ban / hide-not-destroy.

## Forbidden

Pre-show drawers at launch. Visible titlebar. `ensure_opaque`. Hide after show. Svelte restyle. Habitat. `tauri dev`. Stamp. Second writer. Edit `state.json`. Kill Orbitty (tray Quit is Matt).

`./scripts/install-stable.sh` (NOT `tauri dev`). Stop when exit 0. Log what changed. Do not stamp.
