# man_base_32

## Source
- File: `refs/0f1cc01cee35e4f15f7e3c2f1aea8592e5cf1b3460c4c9708f5e8a3c9291a5bd.jpeg`
- Character: adult farmer, black hair, white shirt, blue overalls with yellow buttons, brown shoes
- Pose: side-view walk, facing left, mid-stride (front leg forward, rear arm back)
- Why this sheet: lowest JPEG mush of the adult-man candidates; clear limb separation; classic 16-bit farm-sim silhouette
- Skipped: `d3a19f70…` (cats only); other sheets had similar men but noisier JPEG / less distinct farmer read

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[552, 24, 720, 344]`
- Size: 168×320
- Magenta sheet background ≈ #A35072 keyed to alpha 0; magenta-tinted JPEG fringe dropped

## Process
- 8×8 median grid-snap (phase 0,0) → 21×40 logical pixels
- Nearest-neighbor scale to 16×30, centered, feet 1px above canvas bottom (row 30) so a 1px outline can sit on row 31
- Hard 4-connected 1px outline `#0C080E`
- Quantize / merge near-blacks → 16 entries including transparency (within 16–32)

## Palette (order of first appearance, transparency first)
- transparent #00000000
- #0C080E
- #1B202D
- #E39966
- #7E3A17
- #F9B281
- #C1AE99
- #74574A
- #0F1F49
- #E1D0BC
- #162A59
- #224887
- #917D6A
- #1E3970
- #F7EAD8
- #4B190A

## Quality note
Source is an AI JPEG of “pixel-looking” art, not a real indexed sprite sheet. Grid-snap + NN recovers a readable 16-bit-ish farmer walk, but it is a downsample from mush: some facial detail and the yellow bib buttons collapse or smear, and a few interior outline pixels are thicker than 1px. Not animation-ready without a hand redraw. Transparent background is real (hard alpha 0/255, no fringe).

## Recolor variants (v1)

Exact pixel remaps of `out/man_walk_v1.png` (no new poses). Outline `#0C080E` never remapped. Skin `#E39966` `#F9B281` and shoes `#7E3A17` `#4B190A` unchanged. Hair fill `#1B202D` remapped only in the hair band (`y % 32 <= 4`) for the green variant.

### New master-palette colors (8; total 24 including transparent, ≤32)
- `#3D121C` `#5E1C2A` `#8A283C` `#B63C50` — burgundy overalls
- `#1A2814` `#334822` `#5A7034` — olive/forest overalls
- `#C8A45C` — sandy/blonde hair fill

### man_walk_red_v1
Black hair, white shirt. Overalls:
- `#0F1F49` → `#3D121C`
- `#162A59` → `#5E1C2A`
- `#1E3970` → `#8A283C`
- `#224887` → `#B63C50`

### man_walk_green_v1
Olive overalls, sandy hair, white shirt.
- `#0F1F49` → `#1A2814`
- `#162A59` → `#1A2814`
- `#1E3970` → `#334822`
- `#224887` → `#5A7034`
- hair fill `#1B202D` (y≤4 in each frame) → `#C8A45C`

### man_walk_brown_v1
Brown/tan overalls, dark hair, dusty/tan shirt (existing palette only):
- `#0F1F49` → `#74574A`
- `#162A59` → `#74574A`
- `#1E3970` → `#917D6A`
- `#224887` → `#C1AE99`
- shirt `#F7EAD8` → `#E1D0BC`
- shirt `#E1D0BC` → `#C1AE99`

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).

# woman_base_32

## Source
- File: `refs/0f1cc01cee35e4f15f7e3c2f1aea8592e5cf1b3460c4c9708f5e8a3c9291a5bd.jpeg`
- Character: adult woman, shoulder-length brown hair, yellow/gold dress, dark shoes
- Pose: side-view, facing left, standing / slight step (feet slightly apart)
- Why this crop: same sheet as the man; cleanest adult-woman (not the top-row girl in the red dress, not the grey-bun elder)

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[36, 368, 184, 672]`
- Size: 148×304
- Magenta sheet background ≈ #A34F73 keyed to alpha 0; magenta-tinted JPEG fringe dropped

## Process
- Same as man: 8×8 median grid-snap (phase 0,0) → 18×38 logical pixels
- Nearest-neighbor scale to 14×30, centered, feet 1px above canvas bottom (row 30) so a 1px outline can sit on row 31
- Hard 4-connected 1px outline `#0C080E`
- Quantize onto the locked man palette FIRST. Head (y<11) stays locked-only so skin does not become gold. Yellow dress chroma would have mapped to skin `#E39966` / `#F9B281` (peach dress / man-clone), so two golds were added for fill/shade.

## New palette colors
- `#E59E3D` dress shade
- `#F2C24E` dress fill
- Restored existing recolor extras on `master-palette.png` (`#3D121C` `#5E1C2A` `#8A283C` `#B63C50` `#1A2814` `#334822` `#5A7034` `#C8A45C`) then appended the two golds. Shared total 26 including transparent (≤32).

## Walk
- Cheap 4-frame from interior (outline stripped, then reapplied): F0 base; F1/F3 1px body bob + legs y≥26 collapsed toward midline + 1px near-arm (trailing x≥20); F2 legs reflected across foot midline, arm opposite. Torso/head not redrawn. 128×32 sheet + 170ms loop GIFs. Not imported into Pixelorama.

## Quality note
Same AI-JPEG mush as the man. Grid-snap + NN gives a readable farm-sim woman (brown hair mass, yellow dress, dark shoes) but the long dress collapses into a tapered blob and the face is only a few peach pixels. Walk is pixel-shift only — hem/feet shuffle, no real stride art. Transparent background is real (hard alpha 0/255).

## Recolor variants (v1)

Exact pixel remaps of `out/woman_walk_v1.png` (no new poses, no Pixelorama). Outline `#0C080E` never remapped. Skin `#E39966` `#F9B281` unchanged. Shoes stay `#1B202D` / leftover `#7E3A17` at y≥26. No new master-palette colors (still 26 including transparent, ≤32).

Dress golds remapped globally. Shared mid-brown `#7E3A17` is hair highlight + dress shade + shoe; only the dress band (`12 <= y%32 <= 25`) is remapped so hair and shoes stay brown.

### woman_walk_blue_v1
Brown hair (`#4B190A` `#7E3A17` `#74574A`). Denim dress (man blues):
- `#E59E3D` → `#1E3970`
- `#F2C24E` → `#224887`
- dress-band `#7E3A17` → `#162A59`

### woman_walk_red_v1
Brown hair. Burgundy dress (man reds):
- `#E59E3D` → `#8A283C`
- `#F2C24E` → `#B63C50`
- dress-band `#7E3A17` → `#5E1C2A`

### woman_walk_green_v1
Olive dress (man olives), sandy/blonde hair fill:
- `#E59E3D` → `#334822`
- `#F2C24E` → `#5A7034`
- dress-band `#7E3A17` → `#1A2814`
- hair dark `#4B190A` → `#C8A45C`
- hair highlight `#7E3A17` (y%32 < 12) and `#74574A` left as brown streaks

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).

# dog_base_32

## Source
- File: `refs/8767ae834a2b2967603683e1634850754f7758862123931e101a964401816db2.jpeg`
- Character: brown farm dog (lab/retriever read), blue collar, cream/white tail tip, dark floppy ear
- Pose: side-view standing, facing right, tail up
- Why this crop: cleanest isolated side-view farm dog across the 5 non-cat sheets. Same-sheet-as-man (`0f1cc01c…` r2c0) merges a second animal on the right. `872c25f7…` top-right beagle is holey/outline-heavy after snap. `d3a19f70…` skipped (cats only).

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[40, 704, 256, 864]`
- Size: 216×160
- Magenta sheet background ≈ #913261 keyed to alpha 0; magenta-tinted JPEG fringe dropped

## Process
- Same as man/woman: 8×8 median grid-snap (phase 0,0) → 27×20 logical pixels
- Nearest-neighbor scale to 30×22, centered, feet 1px above canvas bottom (row 30) so a 1px outline can sit on row 31
- Hard 4-connected 1px outline `#0C080E`
- Quantize onto the locked shared palette FIRST. Mid-fur cluster (~#9E4B14) would all collapse to man shoe `#7E3A17` (flat leather blob, no coat read), so one copper fur fill was added. Shade stays shoe-dark `#4B190A`. Collar maps to existing overalls blues. Tail tip maps to shirt cream `#E1D0BC`.

## New palette colors
- `#9E4B14` fur fill
- Appended on `master-palette.png` after the woman golds. Shared total 27 including transparent (≤32).

## Walk
- Cheap 4-frame from interior (outline stripped, then reapplied): F0 base; F1/F3 1px body bob + rear legs (x<13) dx=-1 and front legs dx=+1; F2 opposite stride, no bob; F3 also 1px rear-leg lift. Torso/head/tail not redrawn. 128×32 sheet + 170ms loop GIFs. Not imported into Pixelorama.

## Quality note
Same AI-JPEG mush as the humans. Grid-snap + NN gives a readable farm-sim dog (raised tail with cream tip, blue collar, four short legs) facing right, but the body is a tapered brown loaf, the face is a few dark pixels, and interior outline is thicker than 1px in places. Walk is pixel-shift only — legs shuffle, no real stride art. Transparent background is real (hard alpha 0/255).

## Recolor variants (v1)

Exact pixel remaps of `out/dog_walk_v1.png` (no new poses, no Pixelorama). Outline `#0C080E` never remapped. Tail-tip cream `#E1D0BC` never remapped (stays readable vs body fur). Leftover JPEG ticks `#C8A45C` `#F9B281` `#C1AE99` on the tail left as-is. Leftover burgundy `#5E1C2A` (5 px/frame on tail/neck/collar edge) treated as coat shade so it does not stay red on black/white.

### New master-palette colors (2; total 29 including transparent, ≤32)
- `#2C2C34` `#484852` — charcoal coat fill/highlight for the black variant

### dog_walk_black_v1
Dark/black coat, blue collar unchanged.
- `#9E4B14` → `#2C2C34`
- `#7E3A17` → `#484852`
- `#4B190A` → `#1B202D`
- leftover `#5E1C2A` → `#1B202D`

### dog_walk_white_v1
Cream/white coat, burgundy collar (man reds). Body fill is shirt cream `#F7EAD8` so tail-tip `#E1D0BC` stays a distinct mid-cream.
- `#9E4B14` → `#F7EAD8`
- `#7E3A17` → `#C1AE99`
- `#4B190A` → `#917D6A`
- leftover `#5E1C2A` → `#917D6A`
- `#0F1F49` → `#3D121C`
- `#162A59` → `#5E1C2A`
- `#1E3970` → `#8A283C`
- `#224887` → `#B63C50`

### dog_walk_spot_v1
Brown body kept. Cream belly + muzzle (not a dalmatian scatter — those boxes read as markings at 32px). Blue collar unchanged.
- leftover `#5E1C2A` → `#4B190A` (then may become cream if inside a box)
- spatial, per-frame local coords, bob +1 on F1/F3 (`y`):
  - muzzle: `x%32 >= 25` and `9+bob <= y <= 16+bob` — coat `#9E4B14` `#7E3A17` `#4B190A` `#5E1C2A` → `#E1D0BC`
  - belly/side: `6 <= x%32 <= 16` and `21+bob <= y <= 26+bob` — same coat set → `#E1D0BC`

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).

# cat_base_32

## Source
- File: `refs/d3a19f70b526f4c47d59c3f06232389ce1e7483f1fa4ced70ae2308b354f4efc.jpeg`
- Character: ginger farm/house tabby, white muzzle/chest, green-mapped eye, tail up
- Pose: side-view standing, facing right
- Why this crop: cats-only sheet; cleanest isolated 4-leg side-view among the ginger/brown cells after 8×8 snap. Other rows are larger/noisier (whisker mush) or dark navy (keys poorly on this sheet). Skipped non-cat sheets.

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[232, 952, 392, 1112]`
- Size: 160×160
- Sheet background is navy ≈ #161421 (not magenta); keyed by distance ≥36 from modal navy. Magenta fringe path is a no-op here.

## Process
- Same as man/woman/dog: 8×8 median grid-snap (phase 0,0 on the sheet) → 20×20 logical pixels
- Kept 1:1 (already ≤30), centered, feet 1px above canvas bottom (row 30) so a 1px outline can sit on row 31
- Hard 4-connected 1px outline `#0C080E`
- Quantize onto the locked shared palette FIRST. Ginger fill maps to woman gold `#E59E3D`; stripes/shade to dog copper `#9E4B14` / shoe `#7E3A17`; muzzle/chest to shirt cream `#F7EAD8` / `#E1D0BC`; one eye pixel to olive `#5A7034`. Two JPEG accidents (`#B63C50`, `#5E1C2A`) remapped back onto gold/copper. No new colors.

## New palette colors
- None. Shared total still 29 including transparent (≤32).

## Walk
- Cheap 4-frame from interior (outline stripped, then reapplied): F0 base; F1/F3 1px body bob + rear legs (x≤14, y≥24) dx=-1 and front legs (x≥16) dx=+1; F2 opposite stride, no bob; F1 tail tip dy=-1; F3 tail dx=+1. Head/torso not redrawn. Extra rear-leg lift dropped (stacked with bob and ate the feet). 128×32 sheet + 170ms loop GIFs. Not imported into Pixelorama.

## Quality note
Same AI-JPEG mush as the other bases. Grid-snap + NN gives a readable farm-sim ginger tabby (raised tail, white chest/muzzle, four short legs) facing right, but the body is a gold loaf, the face is a few cream pixels plus one olive eye, and the tail is a hooked stub. Walk is pixel-shift only — legs shuffle and the tail ticks, no real stride art. Transparent background is real (hard alpha 0/255).


## Recolor variants (v1)

Exact pixel remaps of `out/cat_walk_v1.png` (no new poses, no Pixelorama). Outline `#0C080E` never remapped. White chest/muzzle `#F7EAD8` `#E1D0BC` never remapped. Olive eye `#5A7034` never remapped. No new master-palette colors (still 29 including transparent, ≤32).

Coat / leftover warms remapped: fill `#E59E3D`, gold hi `#F2C24E`, sandy leftover `#C8A45C`, peach ticks `#E39966` `#F9B281`, copper shade `#9E4B14`, tan leftover `#C1AE99`, mid-brown `#74574A`, shoe-dark `#7E3A17` `#4B190A`.

### cat_walk_gray_v1
Gray/blue-gray coat (dog black grays). White bib kept. Fill uses the light gray so it does not read as the black variant.
- `#E59E3D` `#F2C24E` `#C8A45C` `#E39966` `#F9B281` → `#484852`
- `#9E4B14` `#C1AE99` `#74574A` → `#2C2C34`
- `#7E3A17` `#4B190A` → `#1B202D`

### cat_walk_black_v1
Black coat, white chest/muzzle kept. No `#484852` so it stays darker than gray.
- `#E59E3D` `#F2C24E` `#C8A45C` `#E39966` `#F9B281` → `#2C2C34`
- `#9E4B14` `#C1AE99` `#74574A` `#7E3A17` `#4B190A` → `#1B202D`

### cat_walk_calico_v1
Spatial patches of existing orange + cream + black read cleanly (no cream-white fallback). Large rear blotch, not a 32px speckle.
- white `#F7EAD8` `#E1D0BC` unchanged
- orange zone (forequarters / head / front legs) unchanged
- black zone uses the black-variant ramp above
- per-frame local coords, bob +1 on F1/F3 (`ly = y + bob`):
  - tail / hind / rump: `x%32 <= 14` — coat set → black ramp
  - mid-back saddle: `x%32 <= 17` and `17 <= ly <= 20` — same

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).

# boy_base_32

## Source
- File: `refs/0f1cc01cee35e4f15f7e3c2f1aea8592e5cf1b3460c4c9708f5e8a3c9291a5bd.jpeg`
- Character: farm boy, brown hair, blue shirt and trousers, brown shoes
- Pose: side-view walk, facing left, mid-stride (front leg forward)
- Why this crop: same sheet as man/woman; cleanest isolated side-view child (not the adult farmer, not the green-coat elder). Other sheets have similar kids but noisier JPEG / less distinct from the locked farmer palette
- Skipped: shrinking the adult man; other-sheet boys (red shirts) that would force extra reds

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[32, 24, 192, 344]`
- Size: 160×320
- Magenta sheet background ≈ #A34F73 keyed to alpha 0; magenta-tinted JPEG fringe dropped

## Process
- Same as man/woman: 8×8 median grid-snap (phase 0,0) → 20×40 logical pixels (tight 20×39)
- Nearest-neighbor scale to 12×24 (adults were ~16×30) so the kid reads shorter; centered, feet 1px above canvas bottom (row 30) so a 1px outline can sit on row 31
- Hard 4-connected 1px outline `#0C080E`
- Quantize onto the locked shared palette FIRST. Bright source shirt-blue (~#4277C9) maps to man overalls `#224887` / `#1E3970` (no new blue). Mid-peach that would have become woman gold is forced to skin `#E39966` / `#F9B281`. Hair-band burgundy ticks remapped to `#4B190A`.

## New palette colors
- None. Shared total still 29 including transparent (≤32).

## Walk
- Cheap 4-frame from interior (outline stripped, then reapplied): F0 base; F1/F3 1px body bob + legs y≥26 collapsed toward midline + 1px near-arm (trailing x≥19); F2 legs reflected across foot midline, arm opposite. Torso/head not redrawn. 128×32 sheet + 170ms loop GIFs. Not imported into Pixelorama.

## Quality note
Same AI-JPEG mush as the adults. Grid-snap + NN gives a readable farm-sim boy (brown mop, blue outfit, split stride) facing left and sitting shorter than the man (bbox y=6..31 vs man y=0..31), but the face is a few peach pixels, the shirt is denim-dark (shared overalls blues, not the source's bright kid-blue), and a couple of shoe pixels picked up burgundy leftovers. Walk is pixel-shift only — feet shuffle, no real stride art. Transparent background is real (hard alpha 0/255).


## Recolor variants (v1)

Exact pixel remaps of `out/boy_walk_v1.png` (no new poses, no Pixelorama). Outline `#0C080E` never remapped. Skin `#E39966` `#F9B281` unchanged. Shoe leftovers `#3D121C` and shoe `#1B202D` (`y%32 >= 27`) unchanged. Face ticks `#B63C50` unchanged. No new master-palette colors (still 29 including transparent, ≤32).

Clothes blues remapped globally. Torso shadow `#1B202D` (`y%32 <= 26`) remapped with the clothes dark so it does not stay navy on olive/tan.

### boy_walk_green_v1
Olive clothes, brown hair kept (`#4B190A` `#7E3A17`).
- `#0F1F49` → `#1A2814`
- `#162A59` → `#1A2814`
- `#1E3970` → `#334822`
- `#224887` → `#5A7034`
- torso `#1B202D` (`y%32 <= 26`) → `#1A2814`

### boy_walk_red_v1
Burgundy clothes, brown hair kept.
- `#0F1F49` → `#3D121C`
- `#162A59` → `#5E1C2A`
- `#1E3970` → `#8A283C`
- `#224887` → `#B63C50`
- torso `#1B202D` (`y%32 <= 26`) → `#3D121C`

### boy_walk_tan_v1
Brown/tan clothes, sandy hair fill.
- `#0F1F49` → `#74574A`
- `#162A59` → `#74574A`
- `#1E3970` → `#917D6A`
- `#224887` → `#C1AE99`
- torso `#1B202D` (`y%32 <= 26`) → `#74574A`
- hair `#4B190A` `#7E3A17` (`y%32 <= 14`, covers F1/F3 bob) → `#C8A45C`
- shoe `#4B190A` (`y%32 >= 29`) kept

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).

# girl_base_32

## Source
- File: `refs/0f1cc01cee35e4f15f7e3c2f1aea8592e5cf1b3460c4c9708f5e8a3c9291a5bd.jpeg`
- Character: farm girl, blonde bob, red/burgundy dress, dark shoes (the top-row girl skipped in the woman notes)
- Pose: side-view walk, facing left, mid-stride
- Why this crop: woman notes called this cell out; same sheet as man/woman; child proportions (more head, less torso), not the yellow-dress adult, not the grey-bun elder

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[256, 24, 400, 344]`
- Size: 144×320
- Magenta sheet background ≈ #A34F73 keyed to alpha 0; magenta-tinted JPEG fringe dropped

## Process
- Same as man/woman: 8×8 median grid-snap (phase 0,0) → 18×40 logical pixels (tight 18×38)
- Nearest-neighbor scale to 11×24, centered, feet 1px above canvas bottom (row 30)
- Hard 4-connected 1px outline `#0C080E`
- Quantize onto the locked shared palette FIRST. Blonde hair maps to woman golds `#F2C24E` / `#E59E3D`. Bright source dress-red (~#B6221F) would have become dog copper `#9E4B14`, so it is forced onto existing man-reds `#B63C50` / `#8A283C` / `#5E1C2A` rather than adding a 30th color.

## New palette colors
- None. Shared total still 29 including transparent (≤32).

## Walk
- Cheap 4-frame from interior (outline stripped, then reapplied): F0 base; F1/F3 1px body bob + legs y≥26 collapsed toward midline + 1px near-arm (trailing x≥19); F2 legs reflected across foot midline, arm opposite. Torso/head/skirt not redrawn. 128×32 sheet + 170ms loop GIFs. Not imported into Pixelorama.

## Quality note
Same AI-JPEG mush. Grid-snap + NN gives a readable farm-sim girl (gold hair mass, burgundy dress, dark shoes) facing left and shorter than the woman, but the dress is shared burgundy not the source's bright red, the pearl collar mostly collapsed, and the face is a few peach pixels. Walk is pixel-shift only — hem/feet shuffle, no real stride art. Transparent background is real (hard alpha 0/255).

## Recolor variants (v1)

Exact pixel remaps of `out/girl_walk_v1.png` (no new poses, no Pixelorama). Outline `#0C080E` never remapped. Skin `#E39966` `#F9B281` unchanged. No new master-palette colors (still 29 including transparent, ≤32).

Dress burgundies remapped in the dress band (`11 <= y%32 <= 26`). Hair golds `#F2C24E` `#E59E3D` `#9E4B14` `#C8A45C` stay blonde except leftover gold `#E59E3D` inside the dress band. Shoe leftovers (`y%32 >= 27`: `#B63C50` `#8A283C` `#5E1C2A` `#3D121C` `#4B190A` `#C8A45C`) → `#1B202D` so they do not read as pink socks on blue/olive/tan.

### girl_walk_blue_v1
Denim/blue dress, blonde kept.
- dress `#B63C50` → `#224887`
- dress `#8A283C` → `#1E3970`
- dress `#5E1C2A` → `#162A59`
- dress `#3D121C` `#4B190A` → `#0F1F49`
- dress leftover `#E59E3D` → `#1E3970`
- shoes `y%32 >= 27` leftover set → `#1B202D`

### girl_walk_green_v1
Olive dress, blonde kept.
- dress `#B63C50` → `#5A7034`
- dress `#8A283C` → `#334822`
- dress `#5E1C2A` `#3D121C` `#4B190A` → `#1A2814`
- dress leftover `#E59E3D` → `#334822`
- shoes `y%32 >= 27` leftover set → `#1B202D`

### girl_walk_brown_v1
Brown/tan dress, darker brown hair.
- dress `#B63C50` → `#C1AE99`
- dress `#8A283C` → `#917D6A`
- dress `#5E1C2A` `#3D121C` `#4B190A` → `#74574A`
- dress leftover `#E59E3D` → `#917D6A`
- hair `#F2C24E` `#C8A45C` (`y%32 <= 15`) → `#7E3A17`
- hair `#E59E3D` `#9E4B14` (`y%32 <= 15`) → `#4B190A`
- shoes `y%32 >= 27` leftover set → `#1B202D`

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).

# old_man_base_32

## Source
- File: `refs/0f1cc01cee35e4f15f7e3c2f1aea8592e5cf1b3460c4c9708f5e8a3c9291a5bd.jpeg`
- Character: farm elder, white/cream hair and beard, long green overcoat, brown shoes
- Pose: side-view standing / slight step, facing left
- Why this crop: woman notes skipped this cell's neighbour (grey-bun) on the same man/woman sheet; boy notes already called the green-coat elder. Same JPEG as man/woman/kids; isolated full body; grey/white hair is in the source (not a recolor). Other sheets (`5aac27ef…`, `e8ed45f6…`) have similar white-beard elders but face right / noisier or bust-only in the scan band.
- Skipped: recoloring the adult farmer's hair grey; `d3a19f70…` (cats)

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[240, 368, 400, 672]`
- Size: 160×304
- Magenta sheet background ≈ #A34F73 keyed to alpha 0; magenta-tinted JPEG fringe dropped

## Process
- Same as man/woman: 8×8 median grid-snap (phase 0,0) → 20×38 logical pixels
- Nearest-neighbor scale to 16×30, centered, feet 1px above canvas bottom (row 30) so a 1px outline can sit on row 31
- Hard 4-connected 1px outline `#0C080E`
- Quantize onto the locked shared palette FIRST. Forest-green coat (~#2A5B32) maps to existing olives `#334822` / `#1A2814` / `#5A7034` (no new green). Hair/beard maps to shirt creams `#F7EAD8` `#E1D0BC` `#C1AE99`. One leftover burgundy foot tick remapped to shoe `#4B190A`.

## New palette colors
- None for this sprite.

## Walk
- Cheap 4-frame from interior (outline stripped, then reapplied): F0 base; F1/F3 1px body bob + legs y≥26 collapsed toward midline + 1px near-arm (trailing x≥19); F2 legs reflected across foot midline, arm opposite. Torso/head/coat not redrawn. 128×32 sheet + 170ms loop GIFs. Not imported into Pixelorama.

## Quality note
Same AI-JPEG mush as the adults. Grid-snap + NN gives a readable farm-sim old man (white beard, green overcoat, split feet) facing left at adult height, but the coat is shared olive not the source's brighter forest green, the face is a few peach pixels plus a beard blob, and the stoop is only a slight forward lean. Walk is pixel-shift only — hem/feet shuffle, no real stride art. Transparent background is real (hard alpha 0/255).


## Recolor variants (v1)

Exact pixel remaps of `out/old_man_walk_v1.png` (no new poses, no Pixelorama). Outline `#0C080E` never remapped. Skin `#E39966` `#F9B281` unchanged. White/cream hair and beard `#F7EAD8` `#E1D0BC` `#C1AE99` `#917D6A` unchanged. Shoes `#4B190A` `#7E3A17` (`y%32 >= 29`) `#9E4B14` and leg `#1B202D` unchanged. No new master-palette colors (still 31 including transparent, ≤32).

Olive coat remapped globally. One leftover shoe-brown `#7E3A17` tick on the trailing coat (`y%32 <= 22`) remapped with the coat dark so it does not stay brown on blue/burgundy.

### old_man_walk_blue_v1
Denim coat, white hair/beard kept.
- `#1A2814` → `#0F1F49`
- `#334822` → `#1E3970`
- `#5A7034` → `#224887`
- coat leftover `#7E3A17` (`y%32 <= 22`) → `#0F1F49`

### old_man_walk_brown_v1
Brown/tan coat, white hair/beard kept.
- `#1A2814` → `#74574A`
- `#334822` → `#917D6A`
- `#5A7034` → `#C1AE99`
- coat leftover `#7E3A17` (`y%32 <= 22`) → `#74574A`

### old_man_walk_burgundy_v1
Burgundy coat, white hair/beard kept.
- `#1A2814` → `#3D121C`
- `#334822` → `#8A283C`
- `#5A7034` → `#B63C50`
- coat leftover `#7E3A17` (`y%32 <= 22`) → `#3D121C`

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).

# old_woman_base_32

## Source
- File: `refs/0f1cc01cee35e4f15f7e3c2f1aea8592e5cf1b3460c4c9708f5e8a3c9291a5bd.jpeg`
- Character: farm elder, grey bun, long purple coat, dark shoes (the grey-bun cell skipped in the woman notes)
- Pose: side-view standing / slight step, facing left
- Why this crop: woman notes called this cell out; same sheet as man/woman; grey hair is in the source (not a recolor). Other sheets have similar grey-bun elders facing right.
- Skipped: greying the yellow-dress adult; inventing a stoop/cane that is not in the cell

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[472, 368, 648, 672]`
- Size: 176×304
- Magenta sheet background ≈ #A34F73 keyed to alpha 0; magenta-tinted JPEG fringe dropped

## Process
- Same as man/woman: 8×8 median grid-snap (phase 0,0) → 22×38 logical pixels
- Nearest-neighbor scale to 17×30, centered, feet 1px above canvas bottom (row 30)
- Hard 4-connected 1px outline `#0C080E`
- Quantize onto the locked shared palette FIRST. Coat chroma (~#492860) mapped to navy/burgundy (wrong family — blue coat / man-clone), so two source-derived purples were added for fill/shade. Hair maps to dog greys `#484852` `#2C2C34` plus creams. Two olive ticks in the bun remapped to `#484852`; leftover burgundy coat ticks remapped to the new purples.

## New palette colors
- `#4A2860` coat fill
- `#2C1440` coat shade
- Appended on `master-palette.png` after the dog charcoal greys. Shared total 31 including transparent (≤32).

## Walk
- Cheap 4-frame from interior (outline stripped, then reapplied): F0 base; F1/F3 1px body bob + legs y≥26 collapsed toward midline + 1px near-arm (trailing x≥19); F2 legs reflected across foot midline, arm opposite. Torso/head/coat not redrawn. 128×32 sheet + 170ms loop GIFs. Not imported into Pixelorama.

## Quality note
Same AI-JPEG mush. Grid-snap + NN gives a readable farm-sim old woman (grey bun, purple coat, dark shoes) facing left at adult height, but the coat is a two-tone plum loaf (collar/fur collapsed), the face is a few peach pixels, and there is no cane. Walk is pixel-shift only — hem/feet shuffle, no real stride art. Transparent background is real (hard alpha 0/255).

## Recolor variants (v1)

Exact pixel remaps of `out/old_woman_walk_v1.png` (no new poses, no Pixelorama). Outline `#0C080E` never remapped. Skin `#E39966` `#F9B281` unchanged. Grey bun `#484852` `#2C2C34` plus cream/tan hair ticks `#C1AE99` `#917D6A` `#74574A` `#E1D0BC` `#F7EAD8` unchanged (including leftover grey at the hem). Face/shoe `#1B202D` (`y%32 <= 14` or `y%32 >= 27`) unchanged. No new master-palette colors (still 31 including transparent, ≤32).

Plum coat remapped globally. Leftover `#7E3A17` at the collar (`11 <= y%32 <= 14`) remapped with the coat shade. Torso interior `#1B202D` (`15 <= y%32 <= 26`) remapped with the coat dark so it does not stay navy on olive/tan.

### old_woman_walk_blue_v1
Denim coat, grey bun kept.
- `#2C1440` → `#162A59`
- `#4A2860` → `#224887`
- leftover `#7E3A17` (`11 <= y%32 <= 14`) → `#162A59`
- torso `#1B202D` (`15 <= y%32 <= 26`) → `#0F1F49`

### old_woman_walk_green_v1
Olive coat, grey bun kept.
- `#2C1440` → `#1A2814`
- `#4A2860` → `#334822`
- leftover `#7E3A17` (`11 <= y%32 <= 14`) → `#1A2814`
- torso `#1B202D` (`15 <= y%32 <= 26`) → `#1A2814`

### old_woman_walk_brown_v1
Brown/tan coat, grey bun kept.
- `#2C1440` → `#74574A`
- `#4A2860` → `#917D6A`
- leftover `#7E3A17` (`11 <= y%32 <= 14`) → `#74574A`
- torso `#1B202D` (`15 <= y%32 <= 26`) → `#74574A`

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).

# cow_base_32

## Source
- File: `refs/8767ae834a2b2967603683e1634850754f7758862123931e101a964401816db2.jpeg`
- Character: brown-and-cream dairy cow, peach snout/udder, small gold horns, dark hooves
- Pose: side-view standing, facing right
- Why this crop: same sheet as the dog; cleanest isolated side-view cow across the 5 non-cat sheets. `0f1cc01c…` farm row is left-facing and cells bleed into the horse. `5aac27ef…` / `e8ed45f6…` similar but noisier JPEG or leftover row-above feet. `d3a19f70…` skipped (cats only).

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[16, 920, 256, 1144)`
- Size: 240×224
- Magenta sheet background ≈ #923260 keyed to alpha 0; magenta-tinted JPEG fringe dropped. y started at 920 (8-aligned) so leftover dog-row feet above the cow fall off.

## Process
- Same as dog: 8×8 median grid-snap (phase 0,0 on the sheet, min_fg 20) → 30×27 logical pixels after largest-CC
- Nearest-neighbor scale to 27×24, centered, feet 1px above canvas bottom (row 30) so a 1px outline can sit on row 31
- Hard 4-connected 1px outline `#0C080E`
- Quantize onto the locked shared palette FIRST. Hide cream `#EFD1AB` maps to shirt cream `#E1D0BC` / `#C1AE99`. Patches map to dog copper `#9E4B14` and shoe `#7E3A17` / `#4B190A`. Snout/udder maps to man skin `#E39966`. Horns map to woman/sandy gold `#E59E3D` / `#C8A45C`. Leftover burgundy ticks remapped to shoe-dark `#4B190A`. No new colors.

## New palette colors
- None. Shared total still 31 including transparent (≤32).

## Walk
- Cheap 4-frame from interior (outline stripped, then reapplied): F0 base; F1/F3 1px body bob + rear legs (low x) dx=-1 and front legs dx=+1; F2 opposite stride, no bob; F3 also 1px rear-leg lift. Torso/head/tail not redrawn. 128×32 sheet + 170ms loop GIFs. Not imported into Pixelorama.

## Quality note
Same AI-JPEG mush as the dog. Grid-snap + NN gives a readable farm-sim cow (cream hide, brown patches, peach snout/udder, four hooves) facing right, but the body is a spotted loaf, the face is a few peach pixels plus gold horn nubs, and interior outline is thicker than 1px in places. Walk is pixel-shift only — legs shuffle, no real stride art. Transparent background is real (hard alpha 0/255).


## Recolor variants (v1)

Exact pixel remaps of `out/cow_walk_v1.png` (no new poses, no Pixelorama). Outline `#0C080E` never remapped. Peach snout/udder `#E39966` never remapped. Horn/snout golds `#C8A45C` `#E59E3D` never remapped. No new master-palette colors (still 31 including transparent, ≤32).

### cow_walk_black_v1
Black/dark body, cream snout/udder kept.
- hide `#E1D0BC` → `#2C2C34`
- hide shade `#C1AE99` → `#1B202D`
- patches `#7E3A17` `#4B190A` `#74574A` → `#1B202D`
- patches `#9E4B14` → `#2C2C34`

### cow_walk_white_v1
Cream/white body. Brown flank/head patches already read as spatial boxes, so they stay.
- hide `#E1D0BC` → `#F7EAD8`
- hide shade `#C1AE99` → `#E1D0BC`
- patches `#9E4B14` `#7E3A17` `#4B190A` `#74574A` unchanged

### cow_walk_brown_v1
Darker all-brown hide, cream snout kept.
- hide `#E1D0BC` → `#9E4B14`
- hide shade `#C1AE99` → `#7E3A17`
- existing brown patches unchanged

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).

# horse_base_32

## Source
- File: `refs/8767ae834a2b2967603683e1634850754f7758862123931e101a964401816db2.jpeg`
- Character: chestnut farm horse, dark mane/tail, dark hooves
- Pose: side-view, facing right, near-front leg slightly lifted
- Why this crop: same sheet as the dog/cow; isolated full side-view. Other sheets have similar bays but merge with the cow cell or add a blonde mane that would fight the cream hide.

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[256, 896, 512, 1144)`
- Size: 256×248
- Magenta sheet background ≈ #923260 keyed to alpha 0; magenta-tinted JPEG fringe dropped

## Process
- Same as dog/cow: 8×8 median grid-snap (phase 0,0 on the sheet, min_fg 20) → 30×30 logical pixels after largest-CC
- Nearest-neighbor scale to 26×26, centered, feet 1px above canvas bottom (row 30)
- Hard 4-connected 1px outline `#0C080E`
- Quantize onto the locked shared palette FIRST. Chestnut `#913F0F` maps to dog copper `#9E4B14` / shoe `#7E3A17` / `#4B190A` (not man burgundy — raw nearest would have gone wine-red). Mane/tail maps to navy `#1B202D` / charcoal `#2C2C34`. One leftover burgundy tick remapped to `#4B190A`. No new colors.

## New palette colors
- None. Shared total still 31 including transparent (≤32).

## Walk
- Cheap 4-frame from interior (outline stripped, then reapplied): F0 base; F1/F3 1px body bob + rear legs dx=-1 and front legs dx=+1; F2 opposite stride, no bob; F3 also 1px rear-leg lift. Torso/head/mane/tail not redrawn. 128×32 sheet + 170ms loop GIFs. Not imported into Pixelorama.

## Quality note
Same AI-JPEG mush. Grid-snap + NN gives a readable farm-sim bay (dark mane/tail, lifted near-front hoof, four legs) facing right, but the body is a brown loaf and the face is a few dark pixels. Walk is pixel-shift only — legs shuffle, no real stride art. Transparent background is real (hard alpha 0/255).


## Recolor variants (v1)

Exact pixel remaps of `out/horse_walk_v1.png` (no new poses, no Pixelorama). Outline `#0C080E` never remapped. No new master-palette colors (still 31 including transparent, ≤32).

Coat `#9E4B14` `#7E3A17` `#4B190A`. Mane/tail `#1B202D` `#2C2C34`. Hooves are leftover `#4B190A` at `y%32 >= 27` (covers F1/F3 bob).

### horse_walk_black_v1
Black coat, dark mane kept (`#1B202D` `#2C2C34` unchanged).
- `#9E4B14` → `#2C2C34`
- `#7E3A17` → `#484852`
- `#4B190A` → `#1B202D`

### horse_walk_white_v1
Cream/white coat, darker mane kept so it still reads.
- `#9E4B14` → `#F7EAD8`
- `#7E3A17` → `#E1D0BC`
- `#4B190A` → `#917D6A`
- hoof `#4B190A` (`y%32 >= 27`) → `#1B202D`

### horse_walk_palomino_v1
Gold coat, cream mane/tail, dark hooves kept.
- `#9E4B14` → `#F2C24E`
- `#7E3A17` → `#E59E3D`
- `#4B190A` → `#C8A45C`
- hoof `#4B190A` (`y%32 >= 27`) unchanged
- mane `#1B202D` → `#F7EAD8`
- mane `#2C2C34` → `#E1D0BC`

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).

# chicken_base_32

## Source
- File: `refs/8767ae834a2b2967603683e1634850754f7758862123931e101a964401816db2.jpeg`
- Character: cream farm hen, burgundy comb/wattle, gold beak, short legs
- Pose: side-view standing, facing right
- Why this crop: same sheet as the dog/cow/horse; isolated side-view. Other sheets have similar hens (some yellow-body) that would need extra golds. Bottom shadow smear under the feet was clipped after snap.

## Crop box
- Absolute pixels on the 768×1152 JPEG, `[x0, y0, x1, y1)` exclusive: `[528, 912, 736, 1136)`
- Size: 208×224
- Magenta sheet background ≈ #923260 keyed to alpha 0; magenta-tinted JPEG fringe dropped. After snap, last 3 logical rows (ground smear) dropped.

## Process
- Same as dog: 8×8 median grid-snap (phase 0,0 on the sheet, min_fg 20) → 25×25 logical pixels after largest-CC and smear clip
- Nearest-neighbor scale to 20×20, centered, feet 1px above canvas bottom (row 30)
- Hard 4-connected 1px outline `#0C080E`
- Quantize onto the locked shared palette FIRST. Body cream maps to `#E1D0BC` / `#C8A45C`. Comb/wattle maps to existing man-reds `#B63C50` / `#8A283C` / `#3D121C` (no new comb red). Beak/feet map to woman gold `#E59E3D` / `#F2C24E`. No new colors.

## New palette colors
- None for the whole farm set. Shared total still 31 including transparent (≤32).

## Walk
- Cheap 4-frame from interior (outline stripped, then reapplied): F0 base; F1/F3 1px body bob + left foot cluster dx=-1 and right dx=+1; F2 opposite stride, no bob. Body/head/tail not redrawn. 128×32 sheet + 170ms loop GIFs. Not imported into Pixelorama.

## Quality note
Same AI-JPEG mush. Grid-snap + NN gives a readable farm-sim hen (cream loaf, burgundy comb, gold beak, two stub legs) facing right and sitting smaller than the cow/horse, but the comb is shared burgundy not the source's bright red, the legs are a few pixels, and the tail is a hooked stub. Walk is pixel-shift only — feet shuffle and the body ticks, no real stride art. Transparent background is real (hard alpha 0/255).

## Recolor variants (v1)

Exact pixel remaps of `out/chicken_walk_v1.png` (no new poses, no Pixelorama). Outline `#0C080E` never remapped. Gold beak/feet `#E59E3D` `#F2C24E` never remapped. Leftover body dark `#4B190A` and brown ticks `#7E3A17` `#9E4B14` left as-is except when they are comb on the white variant. No new master-palette colors (still 31 including transparent, ≤32).

Body cream/sandy `#E1D0BC` `#C8A45C` `#C1AE99` `#74574A`. Comb `#B63C50` `#8A283C` `#3D121C`.

### chicken_walk_brown_v1
Brown hen, burgundy comb kept.
- `#E1D0BC` → `#9E4B14`
- `#C8A45C` → `#7E3A17`
- `#C1AE99` → `#7E3A17`
- `#74574A` → `#4B190A`

### chicken_walk_black_v1
Black hen, burgundy comb kept.
- `#E1D0BC` → `#2C2C34`
- `#C8A45C` → `#1B202D`
- `#C1AE99` `#74574A` → `#1B202D`

### chicken_walk_white_v1
Lighter cream hen (base was already cream). Comb remapped to rust/brown so it stays a distinct variant and reads on white.
- `#E1D0BC` → `#F7EAD8`
- `#C8A45C` → `#E1D0BC`
- `#C1AE99` → `#E1D0BC`
- comb `#B63C50` → `#9E4B14`
- comb `#8A283C` → `#7E3A17`
- comb `#3D121C` → `#4B190A`

Outputs per prefix: `out/<prefix>.png` (128×32), `out/<prefix>.gif` (32×32, 4×170ms, loop, disposal 2), `out/<prefix>_4x.gif` (128×128 NN).
