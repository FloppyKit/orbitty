# Cursor Ultra — GO-ORBITTY-PLANET-SPAWN

**Model:** `kimi-k3-max` · **one writer**
**Gate:** phrase `GO-ORBITTY-PLANET-SPAWN` after Pixel filled REAL.
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Install:** `./scripts/install-stable.sh` → `~/Applications/Orbitty.app`
**SoT:** `~/.buzz/PLANS/ORBITTY_PLANET_SPAWN_LOCK_v0.md` + this handoff

Habitat only. New worlds around the rail.

## Law

1. Change `default_state` in `src-tauri/src/habitat_mac.rs`. Existing worlds **do not move**.
2. Ring around `main` outer frame. Radius = half rail-width + 80pt.
3. First slot 3 o’clock, then counterclockwise 45°. If rail is on the right third (or 3 o’clock clips `visibleFrame`), start 9 o’clock clockwise.
4. Skip slots that clip `visibleFrame`. Disc size = expanded default radius, not marble 18.
5. Drag-to-place keeps the drop point. Do not retarget it onto the ring.
6. No new persist key. Marble clocks untouched.
7. Dogfood only `~/Applications/Orbitty.app` after tray Quit. AX re-grant.

## Files

- `src-tauri/src/habitat_mac.rs` — `default_state` (+ the insert that passes `existing_on_screen` as the slot index)

`AppHandle` is already in this file (`use tauri::{AppHandle, Emitter, Manager}`). `get_webview_window("main")` + `outer_position` / `outer_size`. Convert into screen-local flipped x/y (same as today’s `panel_frame`). `visible_frames()` already exists.

Fallback if rail/window missing: keep today’s mid-right so nothing panics.

## Forbidden

`tauri dev` · `target/` · STAMP_* · moving old planets · drawers · I-DOT · Sky rename · second writer.

When done: `./scripts/install-stable.sh`; stop. Do not stamp.
