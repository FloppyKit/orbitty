# ACCEPTANCE — GO-ORBITTY-SPLIT-DRAWERS

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

## Must

- `src-tauri/src/drawers.rs` exists. `mod drawers;` in `lib.rs`. `lib.rs` no longer contains `PLUCK_DISTANCE` / `show_drawer` / `clamp_drawer_rect`.
- Companion / Board / Settings **jump to the inboard slot** (no slide — COVER deleted it), follow the rail, pluck, Re-tether (Settings → Drawers).
- Off-screen plucked save clamps: 24px handle on the current display. No pane at y=-618 / hanging fully off-right. state.json does not keep a fully-off frame.
- STARS still the shared 780×520 overlay. Rail stays clickable. Right-click on a planet is not WKWebView Reload.
- No new 404.

## Must not

Behavior change. Restore `slide_into_slot`. Screen-center default. Restyle. `tauri dev`. Spawn `stars`. Stamp from the writer.
