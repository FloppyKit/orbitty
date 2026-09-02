# Cursor Ultra — Orbitty settings + agent/action shortcuts + planet zoom

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`  
Locks: `~/.buzz/PLANS/ORBITTY_SETTINGS_LANES_PLANET_v0.md`

## Settings (gear)
- [ ] **Auto-hide row chrome** (default on): complete-circle, stow arrows, folder+ hidden until `.rowwrap:hover` (or section hover)
- [ ] **Show full titles** (default on): off = single-line ellipsis

Persist `autohideRowChrome`, `showFullTitles` in state.json.

## Actions lane (real, not stub)
`actionShortcuts: [{id,name,command,cwd?}]`
- List + **+ Action** form (name, command, cwd optional)
- Click runs via `/bin/zsh -lc` in cwd or `$HOME` (user-authored only). Surface stdout/err in a small note; don’t crash rail.
- Delete control.

## Agents lane (real shortcuts)
`agentShortcuts: [{id,name,harness,profile,model?,mdPaths[],projectId?}]`
- harness: `hermes` now (`cursor` label ok, implement if `cursor-agent` on PATH)
- Launch: write a temp query file = concat readable **.md** paths (skip missing) + titles-only planet context if projectId set. Then `hermes -p {profile} chat --create-if-missing -Q --query-file …` (or existing launch_coordinator path).
- **+ Agent** form: name, profile (text), model optional, md paths comma-separated, optional project attach.
- Keep Soul as “planet note” under Agents, separate from shortcuts.
- One-line: “Shortcuts open your harness. Orbitty does not run the model.”

## Planet
- Scroll over disc (`scrollWheel:` or `magnify`) → radius clamp ~18–220, persist in worlds state.
- Right-click disc menu: **Minimize** (marble) / **Restore** / **Plant** / **Mushroom** (toggle simple pixel decor) / **Boombox** (toggle). Boombox on + saved file path plays (afplay); **Choose music…** file picker for wav/mp3/mid. Off stops.
- Don’t block rail if audio fails.

Keep Home, Lists hide, Show-Desktop flags, tray.rgba as-is.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# sign LiveSprite Local Codesign → ~/Applications/Orbitty.app
```
Summarize. No nest STAMP.
