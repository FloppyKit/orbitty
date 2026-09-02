Sole writer. Pixel does not edit product files. SoT `~/.buzz/PLANS/ORBITTY_PANE_SPA_LOCK_v0.md`. Acceptance `docs/ACCEPTANCE-ORBITTY-PANE-SPA.md`.

`GO-ORBITTY-PANE-PAINT` CODED 22:36 leftover this. Do not re-issue PAINT / HOLD / SHOW / RAISE / SHEET-* / CLAMP. Do not stamp.

Matt: white tiles now have a black drag banner. No Companion / Board / STARS content.

Live leftover: rail Overlay paints, AXWebArea "Orbitty". Companion (1204,199,520×780) and Lists (1004,140,780×520) on=1 white, AXWebArea kids=0. Document loaded, Svelte did not hydrate.

Do (`lib.rs` + `+page.svelte` only):

A. `on_navigation`: allow any tauri/localhost path with **no query**. Reject only `?…` (`?mode=`). On reject: log, return false, **do not hide**. `on_web_resource_request` non-2xx: log, do not hide drawers.

B. `+page.svelte`: no `getCurrentWindow()` at module top. Label from `__TAURI_INTERNALS__?.metadata?.currentWindow?.label`, else try/catch `getCurrentWindow().label`, else `"main"`. Unknown label → dark slab `#1E1E1E`, never white, never rail chrome in a drawer.

C. Empty AXWebArea (zero children) = not mounted. One reload + one `WebviewUrl::App("index.html")` navigate. Do not hide. Do not loop.

Keep PAINT Visible hidden-title, HOLD never-hide, opaque drawers, rail glass, companion pin-while-shown, COVER, STARS-shares-kanban, `?mode=` ban on create URL, hide-not-destroy, sync orderFront.

./scripts/install-stable.sh (NOT tauri dev). Stop when install-stable exits 0. Do not stamp.
