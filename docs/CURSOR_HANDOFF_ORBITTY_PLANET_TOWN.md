# Cursor Ultra — planet town + Kenney catalog

**ONE writer** · `/Users/dude/goose artifacts/livesprite-tauri`  
READ: `~/.buzz/PLANS/ORBITTY_PLANET_TOWN_v0.md`

Matt dogfood: SunnyLand does nothing; Hero Knight still tiny; no 2.5D control; motion is the old ring-bob; wants Pokémon-town amble + zoom past screen + drag sprites + Kenney instead of itch.

## 1) Scale (must be obvious)
`habitat_mac.rs` draw: **delete** `.clamp(28.0, 48.0)`.
```
let fit = (d.radius * 0.45).clamp(72.0, 240.0) * w.scale;
```
`Being.frame` / `SPRITE_FRAME` for walkers must be that same size (hit-test + badge). At default radius 72 the knight is **~72px**, not 28.

## 2) Hide dead walkers
Claim Sprite picker (`+page.svelte`): only `p.frameCount > 0`. Packs gear can still list 0-frame with a “not a walker” note. Add skip needles `sunnyland`, `sunny-land`, `phaser` in `packs.rs` `SKIP_PARTS`.

## 3) 2D / 2.5D menu
Right-click disc, **checked** items:
- `2D disc` → `projection = "flat"`
- `2.5D` → `projection = "oblique"`
Register `planetSetFlat:` / `planetSetOblique:` on the menu target. Draw path already has `radius_y` / scale — just expose it.

Place items stay: `Place plant (n left)`, `Place mushroom (n left)`, `Place building (n left)`, boombox, music. Stop using old bool checkmarks for plant/mushroom.

## 4) Amble + drag sprites
`RoamState`: `{ x, y, tx, ty, pause_end, facing_left }` disc-local.
`anim_tick`: move toward target (~0.18 units/s); pause; new target in disc `r<0.72`. **Do not** call ring-orbit `layout_beings` for walkers — place them with `disc_to_view`.
`Drag::Walker { project_id, entry_id, ... }`: drag pins `(x,y)`. Click matrix if `moved < 4`.

## 5) Zoom + pan
`MAX_RADIUS` from screen: `1.35 * min(w,h) / 2`. Resize panel on zoom. Empty-disc drag pans when huge. Marbles ignore scroll.

## 6) Buildings
`WorldState.buildings: Vec<DecorItem>`. Budget 1 + 1/3d, cap 3. `DecorKind::Building`. Pixel house (door + roof). Growth 7d (shack → house).

## 7) Kenney catalog (itch UI off)
JSON already at `src/lib/kenney-directory.json` (direct `zip` URLs).
- Gear directory: **Kenney (CC0)** collapsed `<details>`, covers, **Install** GETs the `zip` (allowlist only) → unzip → packs/. PK magic still required.
- New command `install_catalog_pack(pageUrl)` or generalize `install_itch_pack` to accept Kenney zip from that JSON.
- Reject URLs not in the JSON.
- `role: habitat` packs install but stay out of the Sprite picker.
- Import folder/zip unchanged. Do not scrape Kenney beyond the allowlist.

Keep rail, rematch, tray.rgba, opacity, claim sprite picker.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
npm run tauri build
# sign LiveSprite Local Codesign → ~/Applications/Orbitty.app
```
Summarize. No nest STAMP.
