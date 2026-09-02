# Cursor Ultra — Orbitty rail polish (titles, close, chrome, tray)

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

Tray **already fixed** on disk: `src-tauri/icons/tray.rgba` is now a **white silhouette on transparent** from the pixel Orbitty (not the dark JPEG square). Keep `icon_as_template(true)`. Do **not** rebuild tray from the full JPG.

## Matt
1. **Hide app name** on rows. Show **icon + title only**. Tooltip can still have app name.
2. **Rename title** — click-to-edit (or pencil) persists `titleOverrides: { windowId, title }[]` in state.json. Display override in rail + Board; AX/OS title unchanged. Empty override = live AX title.
3. **Circle complete also closes the OS window** (AX close). Still: card done, sprite leaves, points rules. If close refused, still complete the card; don’t panic.
4. **Scrollbar** — overlay, hidden until hover/scroll (`overflow: overlay` / webkit scrollbar 0 default, show on `.scroll:hover` / `:hover`).
5. **No traffic lights.** Hide red/yellow/green. One small **×** in the banner closes/hides the **rail only** (not quit). Quit = tray **Quit Orbitty** only. Keep drag region. Same idea on Lists window (× hides Lists).
6. Tray: use the new `tray.rgba` (already written).

Keep planets, claim, Board, opacity, Hermes/Soul, ⌘⇧O.

## Implement notes
- `close_window(id)` via AX close button press (don’t kill pid).
- After circle-complete: complete card → `close_window` → sync habitat.
- Hide standard window buttons in Rust `setup` for `main` + `kanban`.
- Banner × → `window.hide()` not `exit`.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# codesign LiveSprite Local Codesign → ~/Applications/Orbitty.app
```
Summarize. No nest STAMP.
