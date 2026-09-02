# ACCEPTANCE — GO-ORBITTY-SLIM-PANES

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

## Must

- Cold launch: Activity Monitor shows **one** Orbitty WebContent (the rail), not six. RAM well under today’s ~3GB (order-of-magnitude is the win; don’t chase a number).
- Opening Cloud / Lists / STARS / Settings / Companion still works (creates then shows).
- × hides; opening again is instant (keep-warm, no second decode hitch if you just hid it).
- Companion bounds / first-show trailing-edge still apply.
- Identifier `space.orbitty.mac`. Blessed signed app.

## Must not

- Rail missing at launch
- Destroy on ×
- `tauri dev` leftover
- Spirit hitch “fixed” by a new clock
- Drawers invented
