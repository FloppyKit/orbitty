# Cursor Ultra — itch Install + rail scroll + rail width

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

## 1) itch Install fails (“no download found”)
Most catalog pages are **Name your own price**. They have **no** `data-upload_id` until you skip payment.

Verified JS path (`checkout.min.js`):
1. GET `{page}/purchase` (cookie jar + UA)
2. CSRF from `<meta name="csrf_token" value="…">`
3. POST `{page}/download_url` with `csrf_token` + optional `reward_id` (`application/x-www-form-urlencoded`, credentials)
4. JSON `{ "url": "https://…/download/KEY" }` — **that** page has `data-upload_id`
5. Existing POST `{page}/file/{id}?source=game_download` → zip URL → unzip into `packs/`

Also:
- Still try `data-upload_id` on the listing first (true-free packs like Puny, Sprite Pack 8).
- Bake fallback `uploadId` in JSON when we already know it:
  - puny-characters `8193804`
  - sprite-pack-8 `8480214`
- NYOP: do **not** tell the user to use the browser unless `download_url` + file POST both fail.
- Allowlist still binds. Size cap 40MB. User click only.

## 2) Cannot scroll — content falls off the window
Root: settings live in `.chrome` (`flex-shrink: 0`) and `.scroll` uses `overflow-y: overlay` (broken in WKWebView — often no scrollport). `html/body/.rail` are `overflow: hidden`.

Fix:
- **Banner + Home/Board/lanes stay sticky.**
- **Theme/settings panel moves into `.scroll`** (or chrome after banner becomes the scrollport). Settings + window list must scroll.
- `.scroll`: `flex:1; min-height:0; overflow-y:auto;` — **delete `overlay`**. Thin scrollbar always or on hover (`scrollbar-width: thin`). Wheel must work.

## 3) Rail width
- **Min width** = × + “Orbitty” + gear (padding). Do not shrink smaller.
- **Max width** = padding + “Auto-hide row chrome” + “Show full titles” on one row (right edge of Show full titles = max rail).
- Clock sits between wordmark and gear; when narrow, **gear covers the clock** (clock `min-width:0; overflow:hidden`; gear `margin-left:auto; z-index:1; opaque bg`).
- Set Tauri `minWidth` / `maxWidth` on the main window (logical px). Measure ~160 min / ~360–380 max from the two buttons; tweak to match live buttons, don’t guess huge.

Keep rematch, quiet launch-min, tray.rgba, opacity.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# sign LiveSprite Local Codesign → ~/Applications/Orbitty.app
```
Summarize. No nest STAMP.
