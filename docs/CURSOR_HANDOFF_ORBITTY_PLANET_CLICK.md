# Cursor Ultra — Orbitty: tray + board focus + planet clicks

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

**Tray:** `src-tauri/icons/tray.rgba` is a **new** 44×44 white-on-transparent person/planet/orbit (~25% pixels). Keep `icon_as_template(true)`. Do **not** rebuild from the dark JPEG (that is why it was a white square). Recompile so `include_bytes!` picks it up.

## 1) + To-do / todo title → that card
- Creating a todo or clicking a todo title opens Lists **and selects that card** (detail open if possible).
- `open_kanban` plus emit `orbitty:focus-card` with card id. KanbanPage listens, sets `selected` to the card’s project, `view` list or board, opens detail for that id.

## 2) Right-click window title
Context menu: **View on board** (opens Lists focused on that window’s card; create doing card if missing). Optional second item **Rename** if easy.

## 3) Planet beings — single vs double click
`NSEvent.clickCount` in `mouse_down`/`mouse_up`.

| Target | Single | Double |
|--------|--------|--------|
| **Walker** (stowed, dancing) | Caption only — do **not** restore | Restore + focus (window bigger) |
| **Dot** (window open, not dancing) | Focus window + caption | **Stow/minimize** back to planet |

Caption: short line — app/title + “on planet / stowed” or “open”. Prefer a small **native label** on the planet for ~2s, or emit to rail. Titles only.

Distinguish walker vs dot in hit-test (today both call restore).

Planet disc drag unchanged.

## Keep
Circle-complete still closes window. No app names on rows. × hides rail. Opacity, Hermes/Soul.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# sign LiveSprite Local Codesign → ~/Applications/Orbitty.app
```
Summarize. No nest STAMP.
