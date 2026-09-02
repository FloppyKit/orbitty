# Cursor Ultra — S2.1 dogfood fixes (Matt feedback)

**Model:** `kimi-k3-max` · **One writer**  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`  
**Acceptance:** `docs/ACCEPTANCE-S2.1.md`

Matt dogfood: themes partly work; **cannot assign/claim projects** (UX too obscure); no drag-drop; **no app icons**; **Liquid Glass whites out**; rename to **Clear**. Tray OK. App icon polish later.

## Fixes (all required)

### 1. Claim / projects UX (discoverable)
Today claim is a tiny `→` / `▤` button — Matt couldn't find it.

- Replace with a visible control: text **Claim** / project name chip on each row
- Keep row click = focus; claim control stops propagation
- **Drag-and-drop:** drag window row onto a **project section header** → claim to that project
- Drop on **Unmapped** header → unclaim
- Project sections always visible once any project exists (even empty) so drop targets exist
- "+ Project" stays obvious in list head
- Optional: right-click context menu with project list + Unclaim + New project

### 2. App icons on rows
- Extend `WindowInfo` with `iconBase64: Option<String>` (PNG base64, no data: prefix)
- macOS: `NSRunningApplication.icon` → PNG bytes → base64 (cache by bundle id)
- Frontend: `<img class="app-icon" src="data:image/png;base64,{icon}" />` 18×18 before title
- Fallback: first letter of app name in a small square if no icon

### 3. Rename Liquid Glass → **Clear**
- Display name **Clear**; id `clear` (migrate: if saved themeId is `liquidGlass`, load as `clear`)
- CSS selector `data-theme="clear"`
- Theme picker Special block label can just be "Clear" or "Special" with one swatch

### 4. Fix Clear white-out
Current liquidGlass uses near-white ink + transparent bg → unreadable / washed.

Clear theme must:
- Keep **readable dark or light ink** with solid-enough panel background  
- Prefer: dark translucent charcoal panel (`rgba(20,22,28,0.72)` bg, light ink) + optional backdrop-filter blur  
- OR light frosted with **dark** ink (`#1a1d24`) — never white ink on white/clear
- Test both system light and dark OS: text must stay legible
- Tauri window may need transparent background only for Clear; solid themes stay opaque

### 5. Solid theme contrast pass
- Ensure light themes (paper/cloud/cream/mint) have dark ink and enough panel contrast
- Dark themes keep light ink
- No theme should leave `--ink` near white while `--bg` is near white

### 6. Keep working
S1 list/focus/frontmost/AX empty-state; theme persist; claim persist; no sprites.

## Forbidden
S3 sprites/planets, points, Swift dual-write, Android.

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build
```
Summarize. No nest STAMP (Pixel stamps after Matt confirms).
