# Cursor Ultra — GO-ORBITTY-S1-BITTYS

**Model:** `kimi-k3-max` · **one writer**
**Tree:** `/Users/dude/goose artifacts/livesprite-tauri`
**Do not** edit Swift `~/goose artifacts/livesprite`.
**Do not** redraw or “improve” the blessed pixels.

Read first:
- `docs/ACCEPTANCE-ORBITTY-S1-BITTYS.md`
- `~/.buzz/PLANS/BRIEF_ORBITTY_S1_BITTYS.md`
- `~/.buzz/PLANS/ORBITTY_BITTY_CREW_v0.md`
- `brand/bitty-crew/New Crew/planet-sprites/INDEX.md`

## Art (already blessed — copy, don’t invent)

Source: `brand/bitty-crew/New Crew/planet-sprites/out`

| Kind | Default sheet | Palettes (walk prefix) |
|------|---------------|------------------------|
| boy | `boy_walk_v1.png` | green · red · tan |
| girl | `girl_walk_v1.png` | blue · brown · green |
| man | `man_walk_v1.png` | brown · green · red |
| woman | `woman_walk_v1.png` | blue · green · red |
| old_man | `old_man_walk_v1.png` | blue · brown · burgundy |
| old_woman | `old_woman_walk_v1.png` | blue · brown · green |
| cat | `cat_walk_v1.png` | black · calico · gray |
| dog | `dog_walk_v1.png` | black · spot · white |
| cow | `cow_walk_v1.png` | black · brown · white |
| horse | `horse_walk_v1.png` | black · palomino · white |
| chicken | `chicken_walk_v1.png` | black · brown · white |

Each row is a 128×32 sheet (4×32). Optional idle: matching `*_base_32.png` or frame 0.

**Do not ship:** `man_walk_v1.pxo`, `NOTES.md`, `*.gif`, `*_4x.gif`, `master-palette.png`.

Honest: AI JPEG snapped to pixels; walks are shifts+1px bob, not stride art; will not survive zoom. That is the Season 1 look. Leave it.

## Facing lock (pick one — this is the pick)

`habitat_mac.rs` comment: *“Pack frames ship facing right.”* Roam left applies `scaleXBy: -1`.

`out/` humans (boy girl man woman old_man old_woman) face **LEFT**. Animals face **RIGHT**.

**Ship every pack facing right.** When copying a human sheet, flip **each 32×32 cell** independently (not the whole 128×32 as one bitmap). Same for `*_base_32.png` if used as idle. Do not flip animals.

## Pack layout

Prefer the existing engine path (`packs.rs` + `seed_bible_packs` + sheet slice):

```
brand/bitty-crew/packs/orbitty-bitty-<kind>[-<palette>]/
  manifest.json
  walk.png          # 128×32, facing right
  # or walk_0.png … walk_3.png
```

Slug examples: `orbitty-bitty-boy`, `orbitty-bitty-boy-green`, `orbitty-bitty-cat-calico`.

`manifest.json` may be a sheet:

```json
{ "name": "Boy", "sheet": { "file": "walk.png", "frameW": 32, "frameH": 32, "count": 4 } }
```

Bundle via `src-tauri/tauri.conf.json` `resources` (today only `../brand/imagine-walkers/packs`). Add the new packs root. Update `seed_bible_packs` slugs so first launch copies them into `~/Library/Application Support/space.orbitty.mac/packs/`. Existing user packs stay. Old `orbitty-guy` / cat slugs may remain for rematch but **must not** be `default_pack` anymore.

## Product

Default walkers = these bitmaps. `chars::default_pack(look)` returns an `orbitty-bitty-*` slug. Empty `sprite_pack_id` → that slug → habitat plays frames. **Never** fall through to glyph-grid draw as the default path.

### Being editor (default path)

Ship only:
- **Kind** — 11 names above
- **Palette** — that kind’s named recolors (including default/`v1`)
- **Size** — existing 0.4–2.5
- **Use my image…** / Folder — existing local import

**Kill from the default path:** Hair / Clothes / Eyes / Mouth / Body / Build cyclers (`catalog()` slots that talk to `chars.rs` glyph layers). Those looked live and did nothing once Imagine PNGs landed.

Keep persist fields so old claims don’t explode. Map:
- old `species == 0` (person) → `boy` default
- old cat species 1–8 → nearest `cat` palette (orange→default, tuxedo/void→black, calico→calico, gray/cream/siamese→gray)
- ignore clothes/eyes/mouth/hat

Reroll = random Kind + Palette from this table.

User-picked pack still wins over Kind.

Single-click walker → Being editor. Double-click → restore. Unchanged.

## Forbidden

- Redraw / upscale / “fix” the blessed pixels
- Live Imagine designer
- Companion / STARS / EHR / PHI
- Itch / Kenney / Habitica fetch
- Shipping `.pxo` or NOTES into the .app
- New kinds beyond 11
- Editing the Swift `livesprite` tree
- `cargo build --release` as the install (bakes `localhost:1420`, invisible rail)

## Verify (you run; Pixel stamps)

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd "/Users/dude/goose artifacts/livesprite-tauri"
./scripts/install-stable.sh
```

If dmg fails but `src-tauri/target/release/bundle/macos/Orbitty.app` exists, that is enough — the script already continues.

Summarize files changed + residuals. **Do not** write nest `STAMP_ORBITTY_S1_BITTYS.md`. Pixel Artist stamps only after opening `~/Applications/Orbitty.app` and seeing these walkers.
