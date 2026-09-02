# Jacob idle bake — proof 2026-08-27

**Status:** inspect only. Not wired into Orbitty companion UI.  
**Source:** `brand/companion/jacob_src/Jacob.fbx` (copied from Downloads). **Never opened `Jacob.blend`.**  
**License:** CC BY 4.0 — jupifox (credit required if we ship).

## Safety
- CLI: `blender -b -Y --factory-startup -P bake_jacob_idle.py`
- `-Y` = disable autoexec (drivers / startup scripts).
- Script risk lives in the `.blend`. Factory FBX import is data only.

## What this proved
Jacob already has a real `Armature|Idle` clip (208 frames, 1629 fcurves).  
EEVEE bake → 8× **256×384** PNG @ companion size. Motion is **body acting** (tail, weight, sword, head) — not CSS bounce.

This is the factory for other companions:

```
rigged mesh + Idle clip  →  EEVEE bake  →  existing PNG flipbook
```

Live 3D in the spark window stays Phase 2.

## Files
- `frames/idle_0…7.png`
- `preview_strip.png`
- `inspect.json`
- `bake_jacob_idle.py`

## Next (wait Matt)
1. Eyes-on this strip.
2. If blessed: optional pack on disk, still not default spirit.
3. Restyle / own mesh later; keep this bake recipe.
4. Thinking clip = second action, same camera.
