# Cursor Ultra — S2.2 sticky header + Clear theme

**Model:** kimi-k3-max · ONE writer  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`

## Matt dogfood
1. Icons + claim work ✓
2. **Clear theme still bad** — just gray / lifeless
3. **Banner scrolls away** — LiveSprite + clock + gear sit in scroll content. Want **fixed header** that stays while list scrolls. Prefer aligning with titlebar / traffic lights if easy; otherwise hard sticky chrome at top of window.
4. Planets = not yet (S3) — do not build

## Fix A — Sticky chrome (required)
Layout:
```
.rail (flex column, height 100vh, overflow hidden)
  .chrome (flex-shrink 0, sticky top 0, z-index high)
    - padding-top enough to clear traffic lights if using overlay titlebar
    - banner: LiveSprite | clock | gear
    - theme panel when open (also non-scrolling or part of chrome)
    - list-head (+ Project / count) optional in chrome or just under it
  .scroll (flex 1, overflow-y auto)
    - project sections + window rows only
```
- Banner must **never** scroll with window list
- Prefer macOS overlay titlebar: `titleBarStyle: "Overlay"` (or transparent titlebar) so wordmark sits on the same bar as close/minimize; pad left for traffic lights (~72–78px)
- If overlay is flaky, keep decorations and still pin chrome with CSS sticky/fixed

## Fix B — Clear theme (required)
Not gray sludge. Two acceptable looks:
- **Frosted dark glass:** deeper charcoal with real blur if transparent window works, accent `#5AC8FA`, ink near white, edges soft
- **OR frosted light glass:** pale translucent white wash + **dark ink** (#1a1d24)

Must not read as flat gray rectangle. If webview blur fails over desktop, fall back to solid midnight-like panel with name still **Clear** and a one-line residual in footer when Clear is active — still ship readable UI.

Ensure tauri window transparent only helps Clear; solid themes should paint opaque body so they don't look broken.

## Keep
Claim/drag/icons/S1 focus/AX. No sprites. Tray glyph.

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build   # .app is enough if dmg flakes
```
Summarize. No nest STAMP.
