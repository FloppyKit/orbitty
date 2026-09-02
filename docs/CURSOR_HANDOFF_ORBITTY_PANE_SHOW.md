Sole writer. Pixel does not edit product files. SoT `~/.buzz/PLANS/ORBITTY_PANE_SHOW_LOCK_v0.md`. Acceptance `docs/ACCEPTANCE-ORBITTY-PANE-SHOW.md`.

`GO-ORBITTY-PANE-RAISE` CODED 20:38 leftover this. Do not re-issue PANE-RAISE / SHEET-FRONT / SHEET-CLICK / CLAMP. Do not stamp.

Matt cannot see Board / STARS / Companion on any of three monitors. Lists CG on=1 at (1140,189,780,520) is empty Overlay glass (screenshot of that rect is the app underneath; AXWindow has no AXWebArea). Companion CG on=0 overlapping the rail, then hidden. Rail Overlay glass still paints.

Show means a person can read the UI. CG on=1 of empty glass is a fail.

`src-tauri/src/lib.rs` + `src-tauri/src/glass.rs`. Svelte (`src/routes/+page.svelte` overlay branch) only if an opaque window still has no AXWebArea after show.

A. kanban / companion / settings: `transparent: false`. Do NOT run `glass::ensure_transparent` on those labels (that surgery is rail-only). Keep Overlay titlebar, hidden_title, hide_traffic_lights. Opaque NSColor fill so a late SPA is still a visible slab. Do not KVC drawsBackground=NO on drawer webviews. Rail stays transparent Overlay.

B. Companion pin_stay_visible while shown; unpin on hide. kanban/settings stay pinned.

C. Place: 24px handle on some display and not covering the rail → keep. Else inboard slot in POINTS on the rail's display (rail + gap). Never cover the rail — nudge, do not hide. Never 0,0, never screen-center. Then sync AppKit front (keep PANE-RAISE main_thread_sync + activateIgnoringOtherApps + orderFrontRegardless + makeKeyAndOrderFront:). Mouse on when visible + handle + off-rail. Do not hide a visible opaque pane because the CG list lagged.

D. After show, no AXWebArea / empty document → webview reload once. Keep ?mode= ban, STARS-shares-kanban, seed paneMode board, hide-not-destroy, nav-guard on bad URL.

Keep docked inboard, COVER no rail-origin no slide, startDragging, list-row locate-only, 2× heal. Do not gather-button. Do not edit state.json. Do not make the rail opaque.

./scripts/install-stable.sh (NOT tauri dev). Stop when install-stable exits 0. Do not stamp.
