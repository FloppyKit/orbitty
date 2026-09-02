# CURSOR HANDOFF — GO-ORBITTY-SPLIT-DRAWERS

Sole writer. Pixel does not edit product files.

- SoT: `~/.buzz/PLANS/ORBITTY_SPLIT_DRAWERS_LOCK_v0.md`
- Parent: `~/.buzz/PLANS/ORBITTY_SPLIT_LOCK_v0.md`
- Acceptance: `docs/ACCEPTANCE-ORBITTY-SPLIT-DRAWERS.md`

Move the rail leash **and** DRAWER-CLAMP helpers out of `src-tauri/src/lib.rs` into `src-tauri/src/drawers.rs`. Behavior identical. Symbols, not line numbers — CLAMP may still be adding helpers under the banner.

## Do

1. New `src-tauri/src/drawers.rs`. `mod drawers;` in `lib.rs` next to `mod glass;`.
2. Move from `const PLUCK_DISTANCE` through `retether_all_drawers`, including the `// ---------- GO-ORBITTY-DRAWER-CLAMP ----------` block:
   - leash: `DrawerRuntime`, expect/match/clear, `rail_geo` / `rail_visible_frame` / `drawer_slots` / `drawer_slots_inner` / `follow_docked_panes` / persist / `on_drawer_moved` / `wire_drawer_events` / `show_drawer` / `retether_*`
   - clamp: `DRAWER_HANDLE`, `drawer_visible_frame`, `live_rect`, `rect_fully_off`, `rect_off`, `drawer_live_off`, `clamp_drawer_rect`, `drawer_slot_fallback`, `place_plucked`, `persist_drawer_rect`
3. Leave `companion_default_frame` in `lib.rs` as `pub(crate)`. `place_plucked` calls `crate::companion_default_frame`. Export `drawers::bounds_fully_off(app, &rect) -> bool` so `persist_companion_hidden` / companion bounds writer never name `VisibleFrame`.
4. `show_labeled` (still in lib.rs) routes `DRAWER_LABELS` through `drawers::show_drawer`.
5. `ensure_window` calls `drawers::wire_drawer_events`. `run().setup` rail Moved/Resized calls `drawers::follow_docked_panes`. Quit dirty flush calls `drawers::drawer_persist`.
6. `generate_handler!` lists `drawers::retether_drawer`, `drawers::retether_all_drawers`. Frontend invoke names unchanged.
7. Do **not** restore `slide_into_slot`. Docked show jumps to the inboard slot.
8. `cargo check` + `./scripts/install-stable.sh` (NOT `tauri dev`). Blessed `~/Applications/Orbitty.app`.

## Must not change

~48pt pluck, slot order Companion → Board → Settings, STARS shares kanban, Cloud not a drawer, pin_stay_visible, hide-not-destroy, `?mode=` ban, COVER (never rail origin), DRAWER-CLAMP (24px handle, fully-off → inboard slot, never persist off-display, never 0,0, never screen-center).

Chrome laws after this paper was first filled — **move, do not undo**: MATCH Overlay+transparent+`ensure_transparent` (no Visible titlebar, no `ensure_opaque`); SPA query-only never hide + empty WebArea = one reload not a loop; HOLD never-hide after requested show; companion pin-while-shown; GLASS one `--bg-fill` (this slice does not touch Svelte). Beach-ball leftover is a later phrase — not this GO.

Also move with `show_drawer`: `schedule_drawer_mount_check`, `drawer_mount_ok`, `ax_window_has_web_area`, `appkit_order_front`, `pane_mouse_ok`.

## Forbidden

Panes extract. Svelte. Habitat. Restyle. `tauri dev`. Stamp. Second writer. Same-breath SPLIT-PANES. Same-breath beach-ball GO. Restore Visible / `ensure_opaque`. Hide after show.

Stop when install-stable exits 0. Log what moved. Do not stamp.
