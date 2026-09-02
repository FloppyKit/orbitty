Sole writer. Pixel does not edit product files. SoT `~/.buzz/PLANS/ORBITTY_SHEET_FRONT_LOCK_v0.md`. Acceptance `docs/ACCEPTANCE-ORBITTY-SHEET-FRONT.md`.

`GO-ORBITTY-SHEET-CLICK` CODED 18:34 leftover this. Do not re-issue SHEET-CLICK. Do not re-issue CLAMP. Do not stamp.

Reuse SHEET-CLICK helpers in `src-tauri/src/lib.rs` only. Change honor, size, and front.

A. Poisoned size / units
- After honor_rect, if honored w,h is ~2× pane_spec (kanban 780×520, companion 520×780, settings 420×560) or ~2× live size: divide x,y,w,h by 2 and re-honor. Handle on some display → place that. Else slot fallback.
- Companion {2036,448,560,720}: if placing as physical (via current scale) covers the rail or lands on a different display than the points reading, prefer the points reading with a 24px handle. Both fail → slot fallback / companion_default_frame.
- Persist the rect actually placed. Never persist 2× pane_spec. Never persist fully off all displays.

B. Show means AppKit front
- After show+set_focus, if CG on=0 (kCGWindowIsOnscreen / NSWindow isOnActiveSpace — not Tauri is_visible alone): orderFront / makeKeyAndOrderFront on the main thread.
- Still on=0: slot-fallback, place, orderFront again.
- Still on=0: hide + click-through. No shown ghost.
- Already-visible CG on=1 not covering rail → set_focus only.

C. Mouse: only if CG on=1 AND 24px handle on some display AND not covering the rail. Else hide + click-through. Keep ensure_window born click-through. Keep nav-guard.

Keep docked inboard, COVER no rail-origin no slide, startDragging, STARS-shares-kanban, ?mode= ban, hide-not-destroy, pin_stay_visible on non-companion. Do not edit state.json. Do not unpin.

./scripts/install-stable.sh (NOT tauri dev). Stop when install-stable exits 0. Do not stamp.
