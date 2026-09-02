# Face → DX body map v0 (inspiration)

## Bodies (from Downloads, inspiration)
| Blend | Body mesh | Head mesh | Bitty role |
|-------|-----------|-----------|------------|
| `femhumandx` | FemmeBody | head.001 | **female** (+ waist cinch export) |
| `maschumandx` | MascBody | head.001 | **male** |
| `femcaninedx` | FemmeBody | furryhead | **cat** (female canine) |
| `masccaninedx` | mascbody | furryhead | **dog** (masc canine) |
| robot | reuse masc or femme body | robot head plate / future | **robot** |

All ~1.8 unit tall; heads are **separate meshes** — perfect for face swap / decal / replace.

## Banked faces → slots
| Role | Face SoT (2D bank) | 3D head target |
|------|--------------------|----------------|
| female | `ship_v0/heads/head_a_human_…` | femhumandx `head.001` |
| male | `head_a_male_v0` | maschumandx `head.001` |
| cat | cat mouth fuzzy | femcaninedx `furryhead` |
| dog | dog mouth | masccaninedx `furryhead` |
| robot | `head_a_robot_v0` | human body + robot head treatment |

## Pipeline (correct order — Matt)
1. **Bodies as inspiration / proportion lock** (these blends)
2. **Map faces onto heads** (texture project, mesh replace, or render head with face plate)
3. **Then** accessories (hair/hat) in 3D or as 2D on final renders — NOT free-float 2D stickers first

## Waist
- `exports/femhumandx_waist_cinch.blend` + preview
- Optional `femcaninedx_waist_cinch` same factor ~0.86 mid-torso

## Accessories later
Only after one face-mapped turnaround looks right.

## Non-goal
These DX meshes are **inspiration** — we may retopo/simplify for game; AxisLock/DQ8 still governs final look.
