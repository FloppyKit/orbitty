# RETHINK — 🎨 Bot · pixel-artist — 2026-08-27

## Matt: neither UV nor face-card renders are usable. Rethink.

## What failed (stop polishing)
| Attempt | Why dead |
|---------|----------|
| 2D paper-doll auto layers | No shared rig; wrong size/shape |
| VRoid automation | Cursed Unity UI; humanoid-only |
| DX body + UV project | Warped, unreadable FaceLock |
| DX UV island pass 2 | Face didn't stick as art — mesh still wins |
| DX face-card | Readable plate but **card-on-skull**, not a character |

**3D face-map onto these inspiration bodies is not shipping the Bitty look.** Bodies can stay *inspiration for proportion*, not the display path.

## What still has value
- **Banked 2D heads** (female / male / robot / dog / cat roles) — the actual art direction
- **DX blends** — waist, silhouette reference only (optional)
- **Blender factory** — for simple walker blocks / later, not FaceLock decals
- **Orbitty app** — small; build `target/` is disposable cache
- **Product mix** — 5 roles + crazy eyes + hair as *baked looks* or hand layers later

## New default path (proposed)
```
SHIP LOOKS IN 2D (baked full portraits per role/outfit)
  → optional Pixelorama cleanup
  → planet/companion sprites

NOT: 3D mesh + stick face on it until we have a real head made FOR the plate
```

### If 3D returns later
Only with a **head built to FaceLock** (or full character sculpt), not UV-stamping a portrait onto an unrelated doll head.

### Accessories
Hand-authored or gen'd **as complete portrait variants**, not modular stickers — until a real template exists in Pixelorama with anchors.

## Title bug (fixed)
`berd-emoji-titles` used `agent_id` only; this session has `agent_id: null` → always 🪿.
Plugin now infers `pixel-artist` from title `Bot · pixel-artist` → 🎨.

## Do next (pick with Matt)
A. **Baked 2D cast sheet** — one good full-body or bust per role from banked face DNA (Imagine + Pixelorama), no 3D  
B. **Pixelorama-only** paper doll with hand-placed layers on ONE locked guide  
C. **Park character art**; ship Orbitty with existing spirit/bitty-crew packs  

## Stop
- More UV/card experiments on DX heads
- More green-screen accessory factories
- Claiming "usable" when eyes-on says no
