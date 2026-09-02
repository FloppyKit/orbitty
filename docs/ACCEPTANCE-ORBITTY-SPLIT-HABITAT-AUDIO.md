# ACCEPTANCE — GO-ORBITTY-SPLIT-HABITAT-AUDIO

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

## Must

- `src-tauri/src/habitat_mac/audio.rs` exists. `habitat_mac.rs` is now a directory with `mod.rs`.
- `habitat::sync` / `habitat::start` / `habitat::persist_now` still compile as before.
- Boombox still plays. Click play/pause, sprite stays put (~14px slop — slop still on host until INPUT).
- Marble silent; expanded High a bit louder (cap 1.25). Off still kills afplay. Picker still async. No orphan afplay after tray Quit.

## Must not

Change boombox curve. Retune slop. Sibling `src/habitat_audio.rs`. `tauri dev`. Stamp from the writer.
