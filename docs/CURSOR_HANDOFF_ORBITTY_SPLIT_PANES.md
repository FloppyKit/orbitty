# CURSOR HANDOFF — GO-ORBITTY-SPLIT-PANES

Sole writer. Pixel does not edit product files.

- SoT: `~/.buzz/PLANS/ORBITTY_SPLIT_PANES_LOCK_v0.md`
- Parent: `~/.buzz/PLANS/ORBITTY_SPLIT_LOCK_v0.md`
- Acceptance: `docs/ACCEPTANCE-ORBITTY-SPLIT-PANES.md`

Move overlay create/show/hide out of `src-tauri/src/lib.rs` into `src-tauri/src/panes.rs`. Behavior identical to the 12:57 blessed app. DRAWERS already shipped and owns **place** (Law B). This slice does **not** rewrite place.

## Do

1. New `src-tauri/src/panes.rs`. `mod panes;` in `lib.rs`.
2. Move: `emit_pane_vis`, `hide_window`, `pane_spec`, `companion_pin`, `ensure_window` (nav-guard included), `wire_pane_events`, `show_labeled`, `show_shared_pane`, `show_companion`, `companion_default_frame`, `persist_companion_hidden`.
3. `show_labeled` still routes `DRAWER_LABELS` through `drawers::show_drawer`.
4. `drawers.rs` switches `crate::ensure_window` / `crate::emit_pane_vis` / `crate::companion_default_frame` / `crate::companion_pin` → `crate::panes::…`. `crate::main_thread_sync` stays on lib.rs.
5. `lib.rs` keeps `show_rail` and `open_kanban` / `open_stars` / `open_companion` / `open_settings_window` / `open_sky_window` as one-line wrappers (frontend invoke names unchanged).
6. Copy live `ensure_window` flags: `center: true`, transparent Overlay, `hidden_title`, no drag-drop. Drawers: `decorations=false`, `maximizable=false`, `minimizable=false`. URL always `index.html`. Never `?mode=`. Never spawn `stars`. `pane_spec` titles `""`, no stars row.
7. Nav-guard is **query-only**. Non-2xx is **log only**. **Never hide** from nav/resource. HOLD: never hide after a requested show. Hide = banner × / `hide_window` / tray Quit.
8. `companion_default_frame` centers on primary at current size — **no resize, no maximize, no trailing-edge**.
9. `hide_window` / CloseRequested: drawers call `drawers::persist_shown_place` **before** hide.
10. `cargo check` + `./scripts/install-stable.sh`. Blessed `~/Applications/Orbitty.app`.

## Forbidden

Spawn `stars`. Change 780×520. Restore leash / pluck / Overlay title on drawers / hide-on-nav / trailing-edge companion. Drawers extract (already done). Svelte. Habitat. Restyle. `tauri dev`. Stamp. Same-breath SPLIT-RAIL. Strip dead leash symbols. Punch TAG-MAGNET / ATTACH / SKINS / another pane GO.

Stop when install-stable exits 0. Log what moved. Do not stamp. Do not kill Orbitty.
