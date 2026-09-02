# CURSOR HANDOFF — GO-ORBITTY-PANE-STAY

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_PANE_STAY_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-PANE-STAY.md`.

WARM leftover: Board (sometimes Companion) hops far on click. Lists live at CG (-1605,740,894,820). Cause: WARM hidden create has no position (Tauri parks on s2 @2x); `place_logical_rect` uses Tauri `LogicalPosition` which multiplies by the **current** monitor scale, not the destination.

Do (`drawers.rs` + WARM setup loop in `lib.rs` only):

A. `appkit_place_points` — main-thread WAIT `NSWindow setFrame:` in AppKit points. `place_logical_rect` and docked-slot place call it. No Tauri Logical/Physical set_position/set_size for drawer place.

B. After setup `ensure_window` for companion/kanban/settings: park hidden with the same honor-or-slot place. No show. No pin.

C. `show_drawer` already-visible short-circuit: only if a 24px handle is on some display. Mostly-off → place, not focus-only.

Keep WARM / MATCH / SPA / HOLD / GLASS / COVER / STARS-shares-kanban / `?mode=` ban / hide-not-destroy / never-hide / slot-if-fits.

Forbidden: hide after show, Visible titlebar, `ensure_opaque`, screen-center, undo dock, ditch plucked, `slide_into_slot`, stamp, `tauri dev`, `state.json`, kill Orbitty, same-breath SPLIT-PANES.

`./scripts/install-stable.sh` (NOT tauri dev). Stop when exit 0. Do not stamp.
