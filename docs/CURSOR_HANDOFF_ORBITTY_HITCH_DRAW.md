# CURSOR HANDOFF — GO-ORBITTY-HITCH-DRAW

You are the sole product-code writer. Pixel does not edit product files. Stop when install-stable exits 0. **Do not stamp.**

## Do

1. `src-tauri/src/lib.rs` — emit event `orbitty:pane-vis` payload `{ label: string, visible: bool }` from hide (`hide_window`, CloseRequested-to-hide) and from show (`show_labeled`, `show_companion`).
2. `src/lib/CompanionPage.svelte` — listen. `visible:false` (label companion) → `clearInterval(timer)`. `visible:true` → `restartTimer()`. Last frame stays. Do not reload sheets on hide.
3. `src/lib/KanbanPage.svelte` — same event around the 1500ms `refreshWindows` interval.
4. `./scripts/install-stable.sh` (not `tauri dev`). Blessed app `~/Applications/Orbitty.app`.

## Keep

8@140ms art. Reduce Motion hold-frame-0. PANES keep-warm. Overlay transparent. DisplayLink / 16ms / marble hitch untouched. Companion own window. Board and STARS two windows.

## Forbidden

Idle B (freeze when Safari is front). CALayer snapshot. Destroy-on-hide. Companion canvas blit. Native rail. Board+STARS merge. Spirit clock. Gravity poles. `tauri dev`. Second writer. Stamp.

## Stop

Log what changed. No stamp.
