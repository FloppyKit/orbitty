# Cursor Ultra — Orbitty: Lists close, Home, Show Desktop, lanes, tray

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

**Tray:** `src-tauri/icons/tray.rgba` **already redrawn** (larger cube left, smaller planet, tiny stick figure, orbit). `icon_as_template(true)`. Do not touch the JPEG.

## Bugs
1. **Lists × does not close.** Custom × on KanbanPage `hide()` fails. Add Rust `hide_window(label)` (`kanban` / `main`) and call that from ×. Do **not** `prevent_close` without a working hide. × must `stopPropagation` so drag-region doesn’t eat it. Verify Lists actually disappears.
2. **Home** button in rail chrome (next to Board): `orderFront` **all planet panels**. Rust `habitat::bring_all_front()`.
3. **Show Desktop:** rail + Lists + planets `collectionBehavior` = canJoinAllSpaces | stationary | fullScreenAuxiliary; `setCanHide(false)`; `setHidesOnDeactivate(false)`. Honest residual if macOS still tucks them.

## Hermes / Soul / lanes
Matt is confused. **Move Hermes + Soul off every project header** into an **Agents** block (collapsible, bottom of rail or chrome toggle).
- Agents: list “Hermes” (launch) + per-project Soul entries or one “Soul for selected project”.
- **Actions** block: stub — empty state “Actions land here (GO later)” unless a project already has todos (don’t duplicate the whole list).
- Chrome: **Home** | **Board** | optional lane tabs **Projects** (default window list) / **Agents** / **Actions**.
- One-line hint under Agents: “Hermes opens your existing chat app. Soul is a note you Save — Orbitty doesn’t write it for you.”

## Keep
Circle-complete closes window, title rename, planet single/double, opacity.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# sign → ~/Applications/Orbitty.app
```
Summarize. No nest STAMP.
