Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

Poisoned save (do not edit from chat): companion `{x:2036,y:448,w:560,h:720}` visible true; kanban plucked `{x:2280,y:378,w:1560,h:1040}`. Three displays, main @2x.

Must:
- Cold launch rail clickable everywhere. Right-click on a planet is not WKWebView Reload.
- Click Board / STARS / Companion / Settings: pane on a real display, **CG on=1**, in front, 24px handle. Not on the rail. Not 280×360 transparent sheet. Not 1560×1040.
- Kanban size is 780×520 (or user-resized near that), not 2× leftover.
- Companion `{2036,…}` self-heals — either a real 520×780 (or current size) on s1 with a 24px handle, or the inboard slot. Never a 280×360 ghost at logical 1018.
- Mixed points/pixels / 2× size save self-heals on first show and persist writes the placed rect.
- A pane on the right/left portrait stays if a 24px handle is on that monitor.

Must not: undo dock, screen-center, unpin, stamp, edit state.json, kill Orbitty from the writer.
