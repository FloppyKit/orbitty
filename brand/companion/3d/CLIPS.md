# Companion glTF animation clips (reidle-v0)

Authored on chassis GLBs. Bind pose and bone names unchanged. Jaw / mouth / tongue not keyed. Android.glb not touched. Live `/workspace/tang-viewer/assets/{Tang,Girl}.glb` not overwritten.

Job: `orbitty-companion-reidle-v0` — replace hop idle with desk-portrait breath. Matt (CoS chat, 2026-09-03) rejected pelvis-hop idle. think / nod / wave not re-keyed.

Sources (clips-v0 hop, md5 after first clip drop):
- Tang `/workspace/jacob-turnaround/out/tang_clips_v0_hop.glb.bak` md5 `155f25ffab4ae6903f01c05dc310de93`
- Girl `/workspace/jacob-turnaround/out/female_clips_v0_hop.glb.bak` md5 `ec8575cd42f5552f89ec8a7e6b6fe2e5`

Outputs:
- `/workspace/jacob-turnaround/out/tang.glb`
- `/workspace/jacob-turnaround/out/female.glb`

Both files contain glTF `animations[].name`: `idle`, `think`, `nod`, `wave`.

| name | duration | frames @ 24fps | loop | bones keyed | notes |
|------|----------|----------------|------|-------------|-------|
| idle | 4.000 s | 96 (keys 0–96, first=last) | loop | DEF-CHEST (rotation only), DEF-HEAD (rotation only) | desk breath ~2.5° inhale; head drift ≤1.5°; **no DEF-PELVIS01**; no loc hop |
| think | 2.667 s | 64 (keys 1–65, first=last) | loop | DEF-HEAD, DEF-NECK | left as-is from clips-v0; skip re-key (no pelvis bounce) |
| nod | 1.125 s | 27 (keys 1–28, first=last rest) | one-shot | DEF-HEAD | **unchanged** (not re-keyed) |
| wave | 1.333 s | 32 (keys 1–33, first=last rest) | one-shot | DEF-UARM_01.R, DEF-LARM_01.R, DEF-HAND.R | **unchanged** (not re-keyed) |

Wave: **included** on both Tang and Girl.

Export: Blender 5.2.1 `export_animation_mode='ACTIONS'`, `export_force_sampling=False` so unused bones (jaw/tongue) are not baked. Idle keyed as quaternion so exporter keeps nod/think HEAD channels.

Previews: `reidle_tang_rest.png`, `reidle_tang_inhale.png`, `reidle_female_rest.png`, `reidle_female_inhale.png` (pelvis world loc delta = 0; chest tail moves).
