# Cursor Ultra — S2.3 dogfood blockers + system themes

**Model:** kimi-k3-max · ONE writer  
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`

## Matt (blocking)
1. **Two LiveSprite titles stacked** — OS title + in-app wordmark
2. **Cannot move the window** — no working drag surface (stuck)
3. **Drag-to-claim broken** — can drag rows but drop on project does not claim
4. **Clear still gray**
5. **Themes (NEW):** Light / Dark / Clear must **mimic host OS defaults** (macOS system materials & semantic colors; Linux desktop later). Do **not** invent primary CSS palettes for those three. Optional accent palettes (Paper, Mint, etc.) may stay secondary.

Keep: icons, claim button, sticky chrome, no sprites.

## Fix 1 — Single title
- `tauri.conf.json`: `"title": ""` so overlay bar does not paint a second LiveSprite
- Keep in-app `h1` LiveSprite only

## Fix 2 — Window move (required)
- `data-tauri-drag-region` on banner + JS `getCurrentWindow().startDragging()` on mousedown (not on gear/inputs)
- Expand drag hit area beside traffic lights

## Fix 3 — Drag-to-claim works
- dragover preventDefault even without Svelte dragWindowId (use dataTransfer types)
- Large drop targets on section heads; obvious highlight
- + Project first path clear

## Fix 4 — System-faithful themes + Clear

### Primary themes (first in picker)
| id | Name | Behavior |
|----|------|----------|
| `systemLight` | **Light** | Match macOS light window/sidebar/label colors — system semantic / native materials, not custom “Paper” invent |
| `systemDark` | **Dark** | Match macOS dark menu/window colors |
| `clear` | **Clear** | System translucent / vibrancy (window-vibrancy / NSVisualEffect) if possible |

**Default:** follow OS → systemLight or systemDark.

**Migrate on load:**
- `liquidGlass` → `clear`
- Optional: map `paper`→systemLight, `menuCharcoal`→systemDark when loading old state OR keep old ids under secondary “Palette” group

### Implementation (macOS first)
1. Prefer native: Rust `window-vibrancy` for Clear; opaque system-ish backgrounds for Light/Dark
2. CSS for Light/Dark: use colors close to Apple HIG (window bg, control bg, label, secondaryLabel, separator, accent) — or `light-dark()` / media queries tied to theme choice
3. Do **not** make Light = Paper cream or Dark = custom plum as the main path
4. Secondary “Palette” group may keep Paper/Cloud/Cream/Mint/Midnight/Plum/Terminal as fun extras
5. Linux later: Adwaita/Breeze neutrals; document residual

### Clear
- Real vibrancy under transparent webview preferred
- Fallback solid frost with character + footer note — never flat gray, never white-out
- Leaving Clear restores opaque window if needed

## Verify
```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build
```
Summarize. No STAMP.
