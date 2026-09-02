# ACCEPTANCE — GO-ORBITTY-SPRITE-HUG

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant if asked.

## Must

- Idle collapsed, R-click Full/Head: sprite gone, **slash on the icon**, window **shrinks** so the top is the CC / dock — empty air above is gone. Dock stays where it was (grows/shrinks from the top).
- R-click again: sprite back, window returns to the tall idle, slash gone.
- Expand while hugged: transcript is a real slab, not a 120px strip.
- Dest Launchpad while hugged: well is the host frame (HOST-FIT), not a bar. Dismiss: idle hugs again on **that** monitor (HOST-STAY).
- Quit while hugged, reopen: sprite still off, still hugged — not a tiny window with the sprite forced on.

## Must not

- Persist bar as the new idle size
- Hug expanded or hosted
- Frontend `set_size`
- Gear-dock / walker / action-edit / dest-chrome revert
- `tauri dev` / stamp
