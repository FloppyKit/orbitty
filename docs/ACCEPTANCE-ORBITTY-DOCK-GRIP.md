# ACCEPTANCE — GO-ORBITTY-DOCK-GRIP

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant if asked.

## Must

- Idle collapsed: hover the **bottom / bottom-corner / side-at-dock** resize arrows — the window resizes. No click-through to the desk.
- Idle collapsed: drag empty chrome on the dock (padding around dests, not a button) — the window moves.
- Dest buttons, agent pick, field, Send, Talk still click as themselves (no accidental drag).
- Empty air above the dock (around the spirit, around CC) still click-throughs. Spirit still drags.
- Expand / dest Launchpad still full windows.

## Must not

- Click-through on the dock's window-edge
- Drag starting from the text field or a dest button
- Click-through on expanded or hosted
- `tauri dev` / stamp / walker-menu
