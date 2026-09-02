# ACCEPTANCE — GO-ORBITTY-SHEET-CLICK

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

Poisoned save (do not edit from chat): companion `{x:2036,y:448,w:560,h:720}` visible true; kanban plucked may be `{x:2280,y:378,w:1560,h:1040}` (physical leftover) or older `{x:1920,y:-627}`. Three displays, main @2x.

## Must

- Cold launch: rail clickable **everywhere**, not just the left strip. Right-click on a planet is **not** WKWebView Reload.
- Click Board / STARS / Companion / Settings: the pane appears **on a real display**, in front, with a 24px handle. Not on top of the rail. Not `on=0`.
- Companion does **not** open as a 280×360 (or any) transparent sheet overlapping the rail.
- Fully-off-all-displays saves (and mixed points/pixels saves) self-heal on first show. state.json keeps the **placed physical** rect, never a fully-off frame.
- A pane on the right/left portrait monitor stays there if a 24px handle is on that monitor — do not yank it to the rail display just because the rail is on s0.
- After Quit + reopen: no Reload overlay, rail fully clickable, panes come forward.

## Must not

- Undo rail-adjacent dock / re-tether.
- Default every pane to screen-center.
- Unpin `pin_stay_visible`.
- Fake titlebar. `tauri dev`. SPLIT. Stamp from the writer.
- Edit `state.json` from the writer.
