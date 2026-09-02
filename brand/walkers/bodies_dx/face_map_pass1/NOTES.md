# Face-map pass 1 — notes

## What we did
- Body: `femhumandx_waist_cinch.blend`
- Face texture: banked `head_a_human_androgynous_v0.jpg`
- Head mesh: `head.001` (largest head)
- Method: smart UV + **UV Project from front camera** + image texture (Closest / pixel)

## Outputs
- `femhuman_face_map_pass1.blend`
- `turns/portrait_*.png`, `full_*.png`, `turn_*.png`
- `pass1_key_strip.png`, `pass1_turn_strip.png`

## Honest quality
UV project from one camera is a **first contact**, not final:
- Face may stretch on sides/top of skull
- Mouth/ear extras may not match plate
- Plate is front-portrait; profile will look weak until better UVs or a head mesh built for the plate

## Pass 2 options (pick later)
1. Manual UV unwrap face island in Blender to match plate better
2. Shrinkwrap / replace head with a simple card+shell using the plate
3. Bake plate only onto front-facing polys (mask back of head solid skin)
4. Recreate low-poly head to FaceLock proportions then bind to FemmeBody

## Success criteria for pass 1
- Pipeline works end-to-end (texture on separate head, body intact, renders out)
- Eyes-on whether face is readable from front
