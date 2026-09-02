Sole writer. Pixel does not edit product files. SoT `~/.buzz/PLANS/ORBITTY_PANE_RAISE_LOCK_v0.md`. Acceptance `docs/ACCEPTANCE-ORBITTY-PANE-RAISE.md`.

`GO-ORBITTY-SHEET-FRONT` CODED 19:35 leftover this. Do not re-issue SHEET-FRONT / SHEET-CLICK / CLAMP. Do not stamp.

`src-tauri/src/lib.rs` only. Reuse SHEET-FRONT helpers. Change the front gate.

Live leftover: companion `{2288,189,520×780}` and kanban `{1140,189,780×520}` exist, CG on=0. Hide gate is why Board/STARS/Companion do nothing.

A. appkit_order_front: main-thread **wait** (not fire-and-forget run_on_main_thread). On that hop: NSApp activateIgnoringOtherApps:YES, orderFrontRegardless, makeKeyAndOrderFront:. No thread::sleep as the front strategy.

B. pane_cg_on / pane_mouse_ok must NOT require CGWindowListCopyWindowInfo(OnScreenOnly). Overlay-transparent drawers miss that list. Front truth = NSWindow isVisible (isOnActiveSpace only if it does not false-fail Overlay / unpinned companion). CG-list may stay as telemetry — it must not hide a pane.

After show + sync orderFront: if NSWindow isVisible is false → one slot-fallback re-place + sync orderFront. Still not visible → hide + click-through. If isVisible and 24px handle and not covering the rail → mouse on. Do not hide a visible pane because the CG list lagged.

C. Keep ensure_window born click-through, nav-guard, points-native honor, 2× heal, LogicalPosition, persist placed points, already-visible focus-only.

Keep docked inboard, COVER no rail-origin no slide, startDragging, STARS-shares-kanban, ?mode= ban, hide-not-destroy, pin_stay_visible on non-companion. Do not pin companion. Do not gather-button. Do not edit state.json. Do not unpin.

./scripts/install-stable.sh (NOT tauri dev). Stop when install-stable exits 0. Do not stamp.
