# Companion 3D path — 🎨 Bot · pixel-artist — 2026-08-27

## What the companion actually is
Large spark window = **PNG flipbook**. 256×384, 8 frames @ 140 ms, nearest-neighbor, hard alpha.
Thinking v9 only on the blessed spirit. `packId` is hard-null after puppet rollback.
**There is no 3D viewport.** Live GLB would be a product rewrite, not an art trick.

## What failed (do not retry)
| Attempt | Why dead |
|---------|----------|
| VRoid + harness | No API, Unity eats AX, humanoid-only, numbers explode |
| UV / face-card on DX bodies | FaceLock art dies on someone else’s skull |
| PS2 OBJ + CSS bounce | Static mesh. Bounce ≠ body acting |
| Mixamo-style auto-weight on a rip | Bone heat fails; SMD had rest pose only |
| Vivi sectional deform | Proved pixels *can* move. Not a clip. Not ours. Not ship |

## How games actually get this look
Two honest pipelines. Both start with a **T-pose mesh that was built to be rigged**.

1. **Realtime 3D** — Mixamo / AccuRIG / Quaternius humanoid → idle clip → engine plays GLB.
2. **Pre-rendered 3D (Octopath / DQ phone / AoE)** — same rig + clip, **EEVEE-bake to sprites**. 3D is the factory. 2D is what ships.

Orbitty today is (2). DQ8 lighting in the reference shot is (2) or a full game engine — not CSS on a still.

OBJ is only easier than flat images **if it already has a skeleton and animation clips**. A ripped `.obj` is a statue.

## Best path (lock)
```
OWN T-pose character (clean topology, Mixamo-able if humanoid)
  → auto-rig ONCE (Mixamo humanoid / AccuRIG)
  → real Idle clip (breath, weight shift, head)
  → EEVEE bake 8–16 frames @ 256×384, film transparent, AxisLock light
  → existing companion PNG pack
```

- **Humanoid companion:** original AxisLock traveler (DQ8 × SoA slight chibi). Quaternius Universal Base (CC0) is a *rig/proportion scaffold only* — restyle mesh + materials, do not ship Quaternius as Orbitty.
- **Dog / cat:** Mixamo will not save you. Separate simple armature, or bake from the walker factory. Do not force furry onto a humanoid auto-rigger.
- **Robot:** humanoid clip + hard-surface materials is fine.
- **Thinking:** a second clip (look-around / fidget), same camera, same bake. Do not invent a bubble.

## Not now
- Live Three.js / model-viewer in the spark window (Phase 2, Cursor GO, after one bake looks right)
- More PS2 rips as puppets
- Stretching FaceLock plates onto foreign heads
- Paper-doll CSS sway

## Planet walkers
Keep the Blender block factory (`sheet_walker_v0.py`). Those are 32px dots. Different pipeline from the portrait companion.

## First proof (when Matt says go)
One original idle — not Vivi — 8 frames, eyes-on, **not wired** until accepted. Then thinking clip. Then pack picker.
