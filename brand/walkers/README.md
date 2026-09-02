# Orbitty planet walkers — Blender factory

**Blender:** 5.2.1 LTS (`/Applications/Blender.app`, `blender` on PATH)  
**Law:** `AXISLOCK.md`  
**Inspiration:** `inspiration/` — how Matt feeds Pixel + seed cast + prompts  

## Render v0 sheets

```bash
cd "/Users/dude/goose artifacts/livesprite-tauri/brand/walkers"
blender --background --python bpy/sheet_walker_v0.py -- \
  --species traveler_m --out out --frames 4 --res 256

blender --background --python bpy/sheet_walker_v0.py -- \
  --species slime_guide --out out --frames 4 --res 256
```

## Exciting, not only usable
v0 meshes are **block stand-ins** to prove multi-species + walk frames.  
Excitement comes from: your refs + prompt bank + next mesh pass (sculpt / M2M inlet / hand art) while **keeping this sheet pipeline**.

## Feed loop
1. Drop refs in `refs/`  
2. Rewrite one line in `inspiration/SEED_CAST_v0.md` north star  
3. Tell Pixel which seed IDs to render  
4. We re-run bpy → Pixelorama snap → planet pack  
