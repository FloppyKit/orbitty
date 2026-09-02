# Cursor Ultra — Orbitty rename + GO-LS-TAURI-CUTOVER

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`

READ: `~/.buzz/PLANS/ORBITTY_RENAME_CUTOVER_LOCKS_v0.md`

Icons **already generated** — do not re-squish the JPGs:
- `src-tauri/icons/icon.png` + `icon.icns` (3D)
- `src-tauri/icons/tray.rgba` (pixel 44×44)

## Must
1. `tauri.conf.json`: `productName` **Orbitty**, identifier **`space.orbitty.mac`**
2. User-visible “LiveSprite” → **Orbitty** (banner, tray Show/Quit, AX strings, coordinator context header, packs README, Lists title “Orbitty Lists”, habitat `Orbitty — {name}`)
3. Site mention: small “orbitty.space” in theme panel or footer (link via `open`)
4. `scripts/install-stable.sh` → `~/Applications/Orbitty.app`; still sign **LiveSprite Local Codesign**
5. On first launch, migrate Application Support `app.livesprite.mac` → `space.orbitty.mac` if dest empty
6. Exclude own windows for both bundle ids
7. `~/goose artifacts/livesprite/SWIFT_ARCHIVE.md` — Swift dogfood deprecated; open Orbitty only
8. After install, remove or leave a one-line note in old `~/Applications/LiveSprite.app` — prefer **mv aside** to `~/Applications/LiveSprite-old.app` so Matt isn’t confused
9. Optional: global hotkey show/hide main window (`Cmd+Shift+O`)

Keep crate names if rename is noisy. Keep S1–S5 behavior.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# sign + cp to ~/Applications/Orbitty.app
```
Summarize. No nest STAMP.
