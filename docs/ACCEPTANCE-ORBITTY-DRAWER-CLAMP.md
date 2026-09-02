# ACCEPTANCE — GO-ORBITTY-DRAWER-CLAMP

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

Current poisoned save (do not edit from chat): kanban plucked y=-618, companion x=2036.

## Must

- Click Board / STARS / Companion / Settings: the pane appears **on the current display**, with at least a 24px handle visible. Not above the menu bar. Not hanging fully off the right.
- Fully-off saved bounds are ignored. Docked fallback is the **inboard slot** next to the rail, not the middle of the screen, not 0,0.
- After Quit + reopen: those off-screen bounds do not come back. state.json does not keep a fully-off frame.
- Rail stays clickable. Right-click on a planet is **not** WKWebView Reload.
- Grab companion and drag: cursor tracks the window, not a screen away. Window cannot walk fully off an edge.

## Must not

- Undo rail-adjacent dock for first-show / re-tether.
- Default every pane to screen-center.
- Fake titlebar. `tauri dev`. SPLIT. Stamp from the writer.
