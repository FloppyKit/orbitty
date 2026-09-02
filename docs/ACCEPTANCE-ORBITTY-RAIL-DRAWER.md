# ACCEPTANCE — GO-ORBITTY-RAIL-DRAWER

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

## Must

- Open Companion / Board / Settings: pane **slides** out of the rail (~240ms). Reduce Motion: it is just there.
- Open STARS: **same** 780×520 overlay as Board (mode swap), not a second window, not a 404 ghost.
- Drag the **rail**: docked panes follow. No drawn string.
- Drag a pane ~48pt off the rail: it **plucks** and stays. Clicking inside Settings does not pluck.
- Settings has Re-tether (per pane and all). Re-tether puts it back in the slot.
- Hide still hides. Tray Quit still quits.
- Planets do not grow a leash.
- Cloud is not a drawer.
- Blessed signed app. Identifier `space.orbitty.mac`.

## Must not

- Child-window clip / minimize-with-rail
- Click-to-pluck
- Scroll-on-a-string changing planet zoom
- Spawn / pin a leftover `stars` 404 overlay
- `tauri dev` leftover

Do not stamp from the writer.
