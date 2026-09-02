# FaceLock v0 — Orbitty walker / cast face

**Status:** locked draft + image pass · 2026-08-26  
**Recipe:** bald blue cel (3749) as **structure chassis**, pushed through 3744 / 3748 / 3750 as filters.

## Matt clarifications (2026-08-26)
- **Skin:** closer to **3744 nighttime pink / mauve** (cool-pink undertone), not warm gold default, not permanent blue.
- **Bald** only because this is a **doll head** — hair + accessories dress it later. Bald ≠ character identity.
- **Pixelation:** take **extra pixel bite** from **3744 + 3748** (chunky dither-ish clusters, stepped lids, blocky mouth).
- **Folds:** take **slightly rough vector folds** from **3750** (faceted brow/cheek/neck planes, ink-ish crease, not smooth airbrush).

## Contenders
| File | Role |
|------|------|
| IMG_3749 | BASE structure + smile soul |
| IMG_3744 | Skin temp + soft pixel eye/mouth + pixel clusters |
| IMG_3748 | Hard pixel budget / plane economy |
| IMG_3750 | Rough facet folds + brow shelf |

## Hybrid formula
```
FACE = 3749_structure_smile
     + 3744_pink_skin + 3744_pixel_clusters + eye_graphic
     + 3748_feature_budget
     + 3750_rough_vector_folds
     + bald_doll_head (hair is later costume)
```

## One-paragraph lock
> Doll head, bald until dressed. Structure and kind cel smile from the blue bald hero.
> Skin is cool nighttime pink/mauve like the soft pixel portrait (not blue, not tan-gold).
> Eyes large and round with simple iris + one highlight; lids and cheeks show **visible pixel
> stepping** (3744/3748), not smooth paint. Brow shelf and cheek/neck use **slightly rough
> vector folds** (3750 faceted creases). Short wedge nose. Medium cel outline. Must read at
> sprite scale and as a large portrait.

## Prompt snip
```
Bald doll head (hair will be added later), front-facing portrait.
Structure and friendly slight smile from bold cel bald hero reference.
Skin tone: cool nighttime pink-mauve like dusk pixel portrait (rosy lavender-grey pink), NOT blue, NOT orange-tan.
Visible pixelation: chunky pixel clusters on lids, cheeks, mouth corners like SNES/JRPG pixel close-ups.
Slightly rough vector-style folds on brow shelf, under eyes, nose side, neck — faceted ink planes not airbrush.
Large round eyes, simple iris, one highlight, no wet VTuber stack.
Short wedge nose, simple ears. Medium-bold cel outline. Plain dark backdrop.
```

## Outputs
`out/facelock/` — generated sheets for eyes-on

## Plate v1 (Matt locked 2026-08-26)
- **Chosen base:** `facelock_plate_pixel_ink_v1b_chunky`
- **SoT plate:** `out/facelock/facelock_plate_v1.png` (+ copy `facelock_plate_v1.png` at walkers root)
- **Contour ink:** darker/thicker on head silhouette, lateral ears, jawline→chin, neck sides
- **Motion:** polygon **glitch** on walk/idle — small facet jitter / vertex wobble (see `bpy/sheet_walker_v0.py` glitch flag and `MOTION.md`)
- **2026-08-26 dialback:** contour was too hard + clipped ears → plate restamped from chunky + soft contour (`facelock_plate_v1_softcontour`); overcooked kept as `facelock_plate_v1_overcooked_contour.png`
- **Ear ink rule:** KEEP black on head silhouette, outer ears, jaw→chin, neck sides. ONLY the ear-root (ear meets head) is light/blended — never strip global contours.

## Acceptance (Matt 2026-08-26 end of day)
- Plate is **good enough** — stop ear/contour micro-edits.
- Ear-root pixel smudge = reject technique; do not repeat.
- **DQ8 black outline** = later **post-process on full doll**, not built into face.
- Clean working base: prefer pre-smudge plate; Matt ref jpg stored as `facelock_plate_v1_ref.jpg`.
- Next: `NEXT_v0.md` → doll bust under plate.

## Reweight (Matt 2026-08-26 evening)
- **Primary ref: IMG_3744 girl** — not entirely generic; protect this DNA.
- **3748 + 3749 bald plates** — do a lot of the generic/repulsion; demote from chassis.
- 3750 = light/folds only.
- Bald doll pipeline paused as SoT for hero.
