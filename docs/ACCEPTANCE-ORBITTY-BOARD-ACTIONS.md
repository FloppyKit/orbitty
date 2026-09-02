# ACCEPTANCE — GO-ORBITTY-BOARD-ACTIONS

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

## Must

- Launchpad sidebar (top of `.k-side`) shows named macros in **both** List and Board, in the **embedded well and** the popped Lists window. Click runs them (`/bin/zsh -lc`). `+` adds. × deletes.
- Next chip focuses the smallest open todo, or is absent when nothing is open.
- List | Board toggle still only two views. Banner still Launchpad when embedded.
- Macros survive restart **and** survive completing a card / other Kanban mutate (same `actionShortcuts` as rail).
- Rail Actions lane still the old stub (until INSPECTOR).

## Must not

- Third tab
- Actions button on companion dest strip
- Empty `actionShortcuts` after a Board edit that wasn't a delete
- ASK-ACTION intercept / HOST-CC overlay in this binary
