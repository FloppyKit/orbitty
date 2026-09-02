# ACCEPTANCE — GO-ORBITTY-TRAY-REMOTE

Dogfood: tray Quit → only `~/Applications/Orbitty.app` → AX re-grant.

## Must

- Left-click tray icon raises companion, does **not** open a menu, does **not** show the rail. Caret in the field (same as ⌘⇧O).
- Right-click, in order: Show Companion / Launchpad / STARS / Settings / Restore All Stowed Windows / Quit Orbitty.
- Launchpad from the tray grows the companion and fills the **left well with Lists** (host), not a second overlay covering the spirit. STARS same well, STARS page. Companion dests still toggle.
- Dock click / Reopen raises companion, not the rail.
- Light/dark tray icon still swaps.

## Must not

- Left click still `show_menu_on_left_click(true)`
- Show Orbitty / Lists / Board labels in the menu
- Inspector item (that is the next GO)
- Launchpad opening via `show_labeled("kanban")` or `show_shared_pane` (the 23:11 overlay)
