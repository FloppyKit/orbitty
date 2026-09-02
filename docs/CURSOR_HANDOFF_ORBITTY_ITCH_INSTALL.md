# Cursor Ultra — Orbitty packs directory: collapse + preview + install-to-packs

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

Matt: the itch character list is too loud. Hide it. Show a **preview**. **Install** must land in app-data `packs/` — do not bounce through ~/Downloads.

## UI (Gear → Packs)
1. Keep Import folder / zip / Reveal.
2. **Directory** is a **closed `<details>`** (or equivalent). Default collapsed. Summary: “itch.cc0 directory”.
3. Inside: Browse itch.io + CC0 pixel-art (open URLs) + a **grid of cards**, not a wall of text links.
   - 40–48px **cover** (`p.cover` from `$lib/itch-directory.json`)
   - title + author
   - **Install** if `installable !== false`
   - small “page” link to open itch
4. After Install: refresh pack list + `reload_pack_frames` so walkers update.

## Install (Rust) — allowlist only
`install_itch_pack(pageUrl: String) -> PackInfo`

**Allow only URLs that appear in `itch-directory.json`.** Reject anything else.

Flow (verified live):
1. GET the page (browser UA + cookie jar).
2. Parse first `data-upload_id` whose nearby name looks like `.zip` if possible, else first upload.
3. POST `{pageUrl}/file/{uploadId}?source=game_download` (empty body). Response JSON `{ "url": "https://itchio-mirror....r2..." }`.
4. GET that URL → temp zip → reuse existing unzip into `packs/<slug>/`.
5. Write/keep manifest with `license: "CC0 (itch page — confirm)"` + source url. Warn-don’t-block.

Timeouts, size cap (~40MB), no panic. User click only — never on launch.

If install fails (login wall, generator page): return a clear error; UI can still open the page.

## Keep
Rematch, quiet launch-min, Home, opacity, tray.rgba. No Petdex bulk. No Swift.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# sign LiveSprite Local Codesign → ~/Applications/Orbitty.app
```
Summarize. No nest STAMP.
