# Step back — 2026-08-26 (Matt correct)

## Verdict
Almost all auto accessories (diff-extract AND green-screen gens) are **wrong size/shape/placement**. Composites on faces look incorrect. **Do not ship them. Do not keep polishing them.**

## What STILL counts (keep)
| Asset | Why |
|-------|-----|
| **BANKED_Bitties_Starter_v0** | Full portraits, 5 roles, eyes/hair as *reference mood* not layers |
| **ship_v0/heads** | female, male, robot, dog, cat — **SoT faces** |
| **FaceLock / plate refs** | Style DNA |
| **UGC_BITTY_STANDARD_v0.md** | Pack idea still right |
| **Two-head + mouth + robot roles** | Product mix still right |

## What does NOT count (quarantined)
| Junk | Why |
|------|-----|
| Diff-extract layers | Face bleed |
| Green-screen floating props | Wrong scale, no shared rig, random placement |
| Square-crop slots | Cut off mouths |
| Auto “composites” | Prove the pipeline is broken, not the cast |

Quarantine folder: `QUARANTINE_bad_layers_*`

## Why it failed (honest)
Paper dolls need a **shared coordinate system**:
- Same head canvas
- Same anchor (eyes, scalp, ears)
- Layers drawn **to that rig**, not invented in empty space or subtracted from photos

AI gens without a locked template = stickers in random sizes.  
Pixelorama cleanup cannot fix a missing rig.

## Step-back product stance
```
KEEP:  5 role HEAD portraits (dress later)
PAUSE: auto hair/hat/eye layer factory
NEXT:  either (1) manual/template layers in Pixelorama on ONE locked head guide
       or (2) ship heads as complete looks (baked hair variants) without modular layers v0
       or (3) body/walk with FULL baked portraits first — modular dress-up is v1
```

## Recommended path (pick one)

### Path 1 — Baked looks first (fastest to planets) ✅ lean
- Each agent = **one finished portrait** (head already has hair/eyes)
- Bank already has many baked previews in ship_v0
- No modular layers until we care
- Variety = pick different banked PNGs, not compositing

### Path 2 — One honest template (slow craft, real paper doll)
- Lock **one** 512×768 (or 256) head A guide in Pixelorama
- Hand-place or hand-draw **3** hairs, **3** eyes, **2** hats on that guide
- Only then batch more
- AI may assist but **must edit from the guide plate**, not free-float green

### Path 3 — Hybrid
- Ship v0 with baked role portraits
- UGC modular packs later when template exists

**Recommendation: Path 1 or 3.** Modular accessories were a month of craft squeezed into a night of gens.

## Stop doing
- More green-screen prop gens
- More face-diff extract
- More “fix” composites that look wrong
- New skulls

## Resume when
Matt picks Path 1 / 2 / 3 (or a blend).
