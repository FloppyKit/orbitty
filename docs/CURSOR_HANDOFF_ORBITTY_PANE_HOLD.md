Sole writer. Pixel does not edit product files. SoT `~/.buzz/PLANS/ORBITTY_PANE_HOLD_LOCK_v0.md`. Acceptance `docs/ACCEPTANCE-ORBITTY-PANE-HOLD.md`.

`GO-ORBITTY-PANE-SHOW` CODED 21:35 leftover this. Do not re-issue PANE-SHOW / PANE-RAISE / SHEET-FRONT / SHEET-CLICK / CLAMP. Do not stamp.

Matt: click Companion / Board / STARS → flash, then gone.

Live leftover (do not edit state.json): rail on s1 at (2172,-314,360,465). Companion and Lists both **on=-1** at (2540,-314) — inboard-right of a 847-wide portrait, then `show_drawer` hid them. `pane_mouse_ok` is still a hide trigger.

`src-tauri/src/lib.rs` only.

A. After show + AppKit front of a chrome drawer, **never hide** from the mouse/placement gate. Delete the `else { hide(); click-through }` on `!pane_mouse_ok`. Nudge if needed, then leave visible, mouse on. Hide only: banner × / hide_window, nav-guard broken URL.

B. Inboard slot only when the full pane spec fits the rail display vis without covering the rail. Else place on the largest display that fits (s0 here), vis origin + DRAWER_GAP. Never hang off the portrait. Never 0,0. Never screen-center. Do not stack labels on one origin.

Keep opaque drawers, rail glass, companion pin-while-shown, COVER, STARS-shares-kanban, ?mode= ban, hide-not-destroy, sync orderFront. Do not ditch drawers. Do not gather-button.

./scripts/install-stable.sh (NOT tauri dev). Stop when install-stable exits 0. Do not stamp.
