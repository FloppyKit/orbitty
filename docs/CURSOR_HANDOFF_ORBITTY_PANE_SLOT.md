# GO-ORBITTY-PANE-SLOT — writer handoff

Sole writer. Pixel does not edit product files. SoT `~/.buzz/PLANS/ORBITTY_PANE_SLOT_LOCK_v0.md`. Acceptance `docs/ACCEPTANCE-ORBITTY-PANE-SLOT.md`.

Matt does not care if a spot is saved. He wants windows that do not jump.

Do (`drawers.rs` + `persist_companion_hidden` in `lib.rs` only):

A. `show_drawer` / `park_warm_drawer`: always `place_docked_slot` (or existing `drawer_slot_fallback` when withheld). Never `place_plucked`. Never honor `companion.bounds` / `drawers.*.bounds`.

B. Keep STAY focus-only: visible + handle on a display + not covering rail → focus, do not re-place.

C. Stop writing plucked rects: `persist_plucked_bounds` no-op; `persist_drawer_rect` no-op; `persist_companion_hidden` `visible=false` only. Live drag this session may move a shown pane; do not persist bounds.

D. On the place path of show, set that label `docked: true`, `dx=0`, `dy=0`, `bounds=None` in the drawers cache and flush. Do not edit `state.json` by hand.

Keep: GRIP Overlay-only drag, STAY setFrame:/park/focus-only, WARM, MATCH, SPA, HOLD never-hide + slot-if-fits else largest display, GLASS, companion pin-while-shown, COVER (no `slide_into_slot`), STARS-shares-kanban, `?mode=` ban, hide-not-destroy, Re-tether.

Forbidden: hide after show, Visible titlebar, `ensure_opaque`, screen-center, undo docked-inboard, ditch windows, restore `slide_into_slot`, Habitat, Boombox, `tauri dev`, stamp, second writer, edit `state.json`, kill Orbitty, same-breath SPLIT-PANES. Re-issue GRIP/STAY/WARM/MATCH/SPA/HOLD/GLASS.

`./scripts/install-stable.sh` (NOT tauri dev). Stop when exit 0. Do not stamp.
