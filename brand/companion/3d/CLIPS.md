# Companion glTF animation clips (v0)

Authored on chassis GLBs. Bind pose and bone names unchanged. Jaw / mouth / tongue not keyed. Android.glb not touched. Live `/workspace/tang-viewer/assets/{Tang,Girl}.glb` not overwritten.

Sources (pre-clip):
- Tang `/workspace/tang-viewer/assets/Tang.glb` md5 `bc63f14e54c69d3c8cb8b0851e471157`
- Girl `/workspace/tang-viewer/assets/Girl.glb` md5 `d04a7c583fa7a0cef873aaa77ff9f66c` (signed Girl v44)

Outputs:
- `/workspace/jacob-turnaround/out/tang.glb`
- `/workspace/jacob-turnaround/out/female.glb`

Both files contain glTF `animations[].name`: `idle`, `think`, `nod`, `wave`.

| name | duration | frames @ 24fps | loop | bones keyed | notes |
|------|----------|----------------|------|-------------|-------|
| idle | 3.000 s | 72 (keys 1–73, first=last) | loop | DEF-CHEST (loc+rot), DEF-PELVIS01, DEF-HEAD | breath + weight shift + tiny head drift |
| think | 2.667 s | 64 (keys 1–65, first=last) | loop | DEF-HEAD, DEF-NECK | small tilt + yaw fidget; hands stay |
| nod | 1.125 s | 27 (keys 1–28, first=last rest) | one-shot | DEF-HEAD | pitch down then back; no jaw |
| wave | 1.333 s | 32 (keys 1–33, first=last rest) | one-shot | DEF-UARM_01.R, DEF-LARM_01.R, DEF-HAND.R | character-right arm, ~40° raise |

Wave: **included** on both Tang and Girl. Peak 40° pose: 0 crushed rest-valid triangles (Tang 0/5052, Girl 0/14315). Degenerate rest faces on Belt/Hair ignored.

Export: Blender 5.2.1 `export_animation_mode='ACTIONS'`, `export_force_sampling=False` so unused bones (jaw/tongue) are not baked.
