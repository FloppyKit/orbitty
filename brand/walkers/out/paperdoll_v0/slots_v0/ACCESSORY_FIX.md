# What's wrong & the fix

## Why accessories looked bad
We built layers by **diffing** a full-face “hair on head” image against a bald head.

That always leaves:
- face/neck bleed
- almost-full-canvas bboxes
- haunted composites

**Diff-extract is not a real paper-doll pipeline.**

## The fix (in progress / partial done)
1. **Generate each prop alone** on flat **#00FF00 green** (no face in the image)
2. **Chroma-key** green → transparent PNG
3. **Stack** on full-frame head (512×768)
4. **Pixelorama** only for: scale/align, fringe cleanup, better face-hole in wigs

## Where files live
| Path | Role |
|------|------|
| `layers_clean/work/*_raw.png` | green-screen gens |
| `layers_clean/{hair,hats,eyes,accessories}/` | keyed clean layers |
| `slots_v0/{hair,...}/` | installed (bad extracts → `_bad_extract_*`) |
| `ship_v0/` + `BANKED_*` | untouched portraits |

## Remaining craft
- Props may not perfectly align to head (scale/position) — Pixelorama nudge
- Hair wigs may still include a bit of green hole junk or wrong scale
- Eyes float as a pair — may need vertical nudge to eye sockets
- Finish remaining gens if log not ALL_DONE (`layers_clean/gen_log.txt`)

## Do not
- Go back to face-diff extract for production layers
- Overwrite banked heads while cleaning accessories
