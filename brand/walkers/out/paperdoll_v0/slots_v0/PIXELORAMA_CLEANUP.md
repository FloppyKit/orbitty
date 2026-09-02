# Pixelorama cleanup checklist

## Already fixed (no Pixelorama needed)
- **Mouth cutoff** — rolled back. Canvas is **512×768** full portrait.
- **ship_v0 / BANKED** — never damaged (832×1248, md5 OK).
- Bad square crops quarantined: `../slots_v0_BAD_CROP_20260826/`

## Open in Pixelorama
1. `heads/head_a.png` as base layer (locked)
2. Add layer from `hair/hair_silver_electric.png` — erase face bleed
3. Add `eyes/eyes_crazy_spiral.png` — erase non-eye crumbs
4. Export cleaned layers over the same filenames

## Priority clean (worst auto-extract bleed)
- [ ] hair_bob_dark
- [ ] hair_silver_electric
- [ ] hair_ponytail_high
- [ ] eyes_crazy_spiral / spark
- [ ] hats that include face chunks

## Rule
NEVER overwrite `ship_v0/` or `BANKED_*` portraits when cleaning slots.
