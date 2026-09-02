# ACCEPTANCE — GO-ORBITTY-SPLIT-HABITAT-TICK

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

## Must

- `src-tauri/src/habitat_mac/tick.rs` exists. `habitat::start` / `habitat::persist_now` still work.
- Walkers still walk. Idle still stands (bob=0). Hitch still 1-2-3.
- Planets still roam. Boombox still loops from `cursor_tick`. Click still does not slide the sprite (~14px). Tray Quit still stops DisplayLink + afplay.

## Must not

Change hitch math. Change idle-still. Retune GAIN. `tauri dev`. Stamp from the writer.
