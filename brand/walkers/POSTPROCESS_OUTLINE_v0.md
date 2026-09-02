# Post-process outline (DQ8 black line) — LATER step

**Do not** bake heavy contour into the face plate.

## Effect wanted
Dragon Quest VIII–style **singular clean black silhouette outline** on the finished doll/sprite after hair, clothes, accessories are on — one continuous outer ink, medium weight, cel-clear (not airbrush smudge, not per-feature over-ink).

## When
After full-body (or full-bust) composite is rendered / packed:
1. Build doll: FaceLock plate + hair + body + outfit  
2. Optional polygon glitch in motion frames  
3. **Then** outline pass: morphological outer contour or Freestyle/pixel outline  
4. Pixelorama snap if needed  

## Implementation sketch
- **2D PNG path:** alpha silhouette → dilate 1px → difference → black ring composite  
- **Blender path:** Freestyle or solidify-invert outline at render  
- Keep ear-root / internal folds as *value breaks*, not second black cages  

## Status
Parked 2026-08-26 until first full doll sheet exists.
