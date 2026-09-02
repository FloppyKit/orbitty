# Cursor Ultra — planet place/grow/roam + sprite scale

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`  
READ: `~/.buzz/PLANS/ORBITTY_PLANET_PLACE_GROW_v0.md`

## A) Pack frames (Hero Knight tiny, SunnyLand glitch)
`art_for` / `collect_images`:
- Skip path parts: `atlas`, `tileset`, `environment`, `collision`, `title-screen`, `press-enter`, `credits`, `loading`.
- If an image `w % h == 0` and `w/h >= 2` (and h is 16–256), treat as a **horizontal strip** of square frames (`Sheet { frameW:h, frameH:h, count:w/h }`). Prefer files named run/walk, then idle.
- Don’t load a whole strip as one frame.
- If a pack has **zero** walk frames after filters, `frameCount=0` and picker can show it disabled.

Draw pack frames **fitted** into ~max(28, min(48, radius*0.28)) box, aspect-correct, nearest-neighbor. Not a forced 36×36 squash.

## B) WorldState (additive, old saves load)
```
projection: "flat" | "oblique"   // 2D / 2.5D
firstSeen: epoch
plants: [{x, y, plantedAt}]      // disc-local -1..1
mushrooms: [{x, y, plantedAt}]
boombox: {x, y} | null
```
Migrate old `plant`/`mushroom` bools to one default placement. Keep `music` + playing flag.

Budget: start 3+3+1; extra plant+mushroom every 2d from `firstSeen`; cap 5+5; boombox 1.

Growth t = clamp((now-plantedAt)/7d, 0, 1). Plant: sprout → tree (scale + extra pixels). Mushroom: 1 cap → 3–5 cluster.

## C) Interaction
Right-click disc: **2D / 2.5D**, Place plant (n left), Place mushroom (n left), Place boombox (if none), Choose music, Minimize/Restore.
After Place X, next click on disc (not on a sprite) drops it. Drag decor to move. Right-click decor = remove (refund).
Boombox click = toggle music if a file is set.

## D) Roam
Walkers orbit the rim (flat) or ellipse equator (oblique) with angular speed. Flip when going left. Use run/walk frames while moving; idle if we have it when paused. Occasional pause 1–2s. Don’t break single/double click.

2.5D: disc is an ellipse (ry ≈ rx*0.55). Sprites on the front arc sit a bit larger.

Keep rail, rematch, claim sprite picker, tray.rgba, opacity.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build || true
# sign LiveSprite Local Codesign → ~/Applications/Orbitty.app
```
Summarize. No nest STAMP.
