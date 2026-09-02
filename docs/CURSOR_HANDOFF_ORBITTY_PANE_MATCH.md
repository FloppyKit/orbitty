Sole writer. Pixel does not edit product files. SoT `~/.buzz/PLANS/ORBITTY_PANE_MATCH_LOCK_v0.md`. Acceptance `docs/ACCEPTANCE-ORBITTY-PANE-MATCH.md`.

`GO-ORBITTY-PANE-SPA` CODED 23:06 leftover this. Do not re-issue SPA / PAINT / HOLD / SHOW / RAISE / SHEET-* / CLAMP. Do not stamp.

Matt: Companion + Board now mount and stay. Extra black native banner above both. Opacity 100% — does not follow Settings (0.75) so they do not match the rail.

Cause: PAINT Visible titlebar + `ensure_opaque` slab alpha 1.0. SPA now hydrates; those levers are leftover.

Do (`lib.rs`; `glass.rs` only if `ensure_glass` still forces opaque):

A. Drawers: `transparent: true`, `TitleBarStyle::Overlay`, `hidden_title`. Same as rail. No extra native strip. Keep `hide_traffic_lights`. In-SPA banners stay.

B. `ensure_transparent` on drawers. Never `ensure_opaque` on a drawer. `ensure_glass` transparent path for drawers. `applyWindowOpacity` already runs — `--bg-fill` must show the desk at the slider.

C. Keep SPA never-hide / query-only / empty-WebArea-reload, HOLD never-hide, companion pin-while-shown, COVER, STARS-shares-kanban, `?mode=` ban on create URL.

./scripts/install-stable.sh (NOT tauri dev). Stop when install-stable exits 0. Do not stamp.
