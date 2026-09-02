# Cursor Ultra — itch zip EOCD + sprite picker + chrome toggles

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

Matt: Install often **EOCD** (not a zip). The 1–2 that landed (Hero Knight) sit in `Sprites/*.png` so walkers never use them. No sprite picker. **Auto-hide row chrome** and **Show full titles** do nothing (classes on `.rail`, **zero CSS**). Add **hide icons**.

## 1) Install EOCD
After GET, require magic `PK`. If HTML/`<`, error “itch sent a page, not a zip”.
NYOP: POST `/file/{id}` against the **download-page URL** (from `download_url`), not only the listing URL. If `download_url` JSON `url` is already a zip, use it.
Do not unzip 7z/rar. Keep 40MB cap.

## 2) Art from nested packs
`art_for` today only reads **top-level** `*.png`. Hero Knight is `packs/hero-knight/Sprites/*.png`.

- Recurse depth ≤4 for png/gif/webp (skip `__MACOSX`, `.idea`, `node_modules`).
- Prefer paths/names with walk/run/idle.
- Cap ~8 frames for a walker. NSImage from bytes (not PNG-only).
- `list_packs` frameCount must go up for Hero Knight.

## 3) Sprite picker = claim menu
Same popover as Home folder + Tags:

```
Sprite
[default] [hero-knight] [other packs…]
```

- Persist `claims[].spritePackId` (pack folder id, empty = built-in).
- Habitat snapshot sends it; walker uses **that** pack, not hash-of-window.
- Unpicked stays built-in (don’t silently steal the first imported pack).
- Small cover if easy (first frame); name is enough.

## 4) Settings CSS (actually work)
`.rail.autohide` (default on): hide `.circle-btn`, `.stow-btn`, `.claim-btn` until `.rowwrap:hover` / `:focus-within`.
`.rail.full-titles` ON: `.wtitle` wraps. OFF: one line + ellipsis (must look different).
**Hide icons** (`hideAppIcons`, persist): hide `.app-icon` (layout collapse).

## Keep
Rematch, quiet launch-min, scroll, 160–360, tray.rgba, opacity.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# sign LiveSprite Local Codesign → ~/Applications/Orbitty.app
```
Summarize. No nest STAMP.
