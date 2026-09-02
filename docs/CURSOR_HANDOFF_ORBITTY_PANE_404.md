# CURSOR HANDOFF — GO-ORBITTY-PANE-404

Sole writer. Pixel does not edit product files. SoT: `~/.buzz/PLANS/ORBITTY_PANE_404_LOCK_v0.md`. Acceptance: `docs/ACCEPTANCE-ORBITTY-PANE-404.md`.

Desk-blocking leftover of BOARD-STARS. Both rail Board and STARS paint a 404 overlay that eats the desktop.

## Do (`src-tauri/src/lib.rs` + maybe `src/routes/+page.svelte`)

1. **Never** put `?mode=` in `WebviewUrl::App`. Cold URL is `index.html`. Persist `shared_pane_mode` **before** show. Warm emit `orbitty:shared-pane-mode`. Cold mount already reconciles via `load_state` in `+page.svelte` — keep that. Seed `paneMode` from `load_state`, not the URL query.
2. Do **not** `ensure_window("stars")`. If a leftover `stars` pane exists, hide it, never pin a new one. Prefer drop `"stars"` from `pane_spec` if nothing else needs the label.
3. If first navigation 404s / custom-protocol miss: **hide**, do not pin a broken overlay.
4. `open_kanban` / `open_stars` stay `show_shared_pane` on the one 780×520 `kanban` overlay.

## Forbidden

Spawn a second STARS window. Destroy sky. Companion merge. Caption. Boombox. Drawers. Clamp. SPIRIT-FLOAT. `tauri dev`. Stamp. Second writer.

`./scripts/install-stable.sh` (NOT `tauri dev`). Blessed `~/Applications/Orbitty.app`. Stop when install-stable exits 0. Log what changed. Do not stamp.
