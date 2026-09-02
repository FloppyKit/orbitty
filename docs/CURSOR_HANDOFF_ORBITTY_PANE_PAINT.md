Sole writer. Pixel does not edit product files. SoT `~/.buzz/PLANS/ORBITTY_PANE_PAINT_LOCK_v0.md`. Acceptance `docs/ACCEPTANCE-ORBITTY-PANE-PAINT.md`.

`GO-ORBITTY-PANE-HOLD` CODED 22:02 leftover this. Do not re-issue HOLD / SHOW / RAISE / SHEET-FRONT / SHEET-CLICK / CLAMP. Do not stamp.

Matt: Companion / Board / STARS now **stay** as permanent **white** windows. No content. Rail still paints.

Live leftover (do not edit state.json): rail Overlay on=1 (618,308,360,420) paints. Companion (8,39,520,780) and Lists (1140,189,780,520) on=1 **white**, AX no WebArea.

Cause: PANE-SHOW `transparent:false` + `TitleBarStyle::Overlay` + `ensure_opaque` (under-page only, never drawsBackground=NO). Overlay+opaque WKWebView on this desk is default white and never mounts the SPA. Same `index.html` paints in the rail because Overlay+transparent.

Do (`lib.rs` + `glass.rs` + drawer-window CSS in `+page.svelte` only):

A. Drawers: `TitleBarStyle::Visible`, `hidden_title`, `transparent:false`. Overlay is rail/sky only. Keep `hide_traffic_lights`.

B. `ensure_opaque`: slab on NSWindow **and** WKWebView `drawsBackground=YES` + under-page = slab. Never `strip_webview_fill` on a drawer. After HOLD show, if still no AXWebArea after reload-once, navigate bare `index.html` once. Do not hide.

C. `+page.svelte`: if kanban/companion/settings, `html`/`body` background `var(--bg-fill, var(--bg, #1E1E1E))`. Rail stays transparent.

D. Seed `themeId` to OS default synchronously in KanbanPage / CompanionPage / SettingsPage so `--bg` exists before `load_state`.

Keep HOLD never-hide, slot-if-fits, opaque drawers, rail glass, companion pin-while-shown, COVER, STARS-shares-kanban, `?mode=` ban, hide-not-destroy, sync orderFront.

./scripts/install-stable.sh (NOT tauri dev). Stop when install-stable exits 0. Do not stamp.
