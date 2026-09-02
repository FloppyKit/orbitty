# What's next — walker cast (after FaceLock)

## Locked
- **FaceLock plate** (doll head, bald, pink-mauve, blue eyes, chunky facet) — good enough; stop micro-editing ears  
- **AxisLock** style law  
- **Blender factory** + `--glitch`  
- **Outline:** post-process later (DQ8 outer ring), not face-bake  

## Pipeline order (exciting path)

### 1. Full-bust / doll under this face (NOW)
- Gen or Blender: neck+shoulders+simple torso matching FaceLock plate  
- Same pink-mauve, pixel/facet language  
- Still bald (hair is dress-up)  
- Output: `out/doll/doll_bust_v0.png`

### 2. Hair + accessory kits (dress the doll)
- 3–5 hair styles (bandana hero, twin-tail, short messy, etc.) as **layers**  
- Optional: bag, scarf  
- Keep separate so one face → many walkers  

### 3. Full-body turn / walk sheet
- Front + side (or 4-dir) body under head  
- Blender sheet script **or** gen strips under AxisLock + FaceLock ref  
- Apply `--glitch` lightly on 3D path  

### 4. DQ8 outline post-pass
- Single black outer line on finished frames (see `POSTPROCESS_OUTLINE_v0.md`)  

### 5. Pixelorama → planet pack
- Scale to walker size, index palette, import Orbitty pack  

### 6. Second species
- Slime / birdfolk with same outline+glitch pipeline (FaceLock is humanoid plate only)

## Non-goals this week
- More ear-pixel surgery  
- VRoid  
- Perfect 1:1 DQ8 copy  

## Recommend next action
**Generate doll bust under FaceLock plate** (hairless), eyes-on once, then hair layer options.
