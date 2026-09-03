# CURSOR HANDOFF — GO-ORBITTY-COMPANION-3D

Model: kimi-k3-max. One writer. Read `~/.buzz/PLANS/ORBITTY_COMPANION_3D_LOCK_v0.md` first.

Tree: `~/goose artifacts/livesprite-tauri`. Blessed app only after `./scripts/install-stable.sh`. Identifier `space.orbitty.mac`.

GLBs already in `brand/companion/3d/{tang,female}.glb` (+ License.txt + CLIPS.md). Clips: **`idle` `think` `nod` `wave`**. This GO plays **`idle`** (loop). `think` only while a send is in flight. Do not MCP `nod`/`wave` (that's PLAY).

## Disk facts (do not re-export)

- Tang: `DEF-HEAD` `DEF-NECK` `DEF-JAW` `ROOT` · meshes `Tang_Body` `Tang_Head` · CC BY jupifox
- Female: same chassis · `Girl_Head` `Girl_Hair` `Companion_Body`
- Extension: `KHR_materials_specular`. Keep materials. Hemisphere + key light only.

## Do

1. **new** `src/lib/Companion3D.svelte`. Canvas in the same slot as `SpiritStage` (idle stage, hosted wellhead, 96×96 corner). `CompanionPage` mounts 3D when `bodyId` is `tang` or `female`, else SpiritStage.

2. `npm` dep `three` **^0.170.0**. `import('three')` **only** when a 3D body is selected. `GLTFLoader` from `three/addons/loaders/GLTFLoader.js`.

3. `tauri.conf.json` resources: `"../brand/companion/3d": "companion/3d"`. Invoke `companion_glb_url` → `convertFileSrc` for `companion/3d/{id}.glb`. Miss → empty string, not a panic.

4. Settings select: Spirit · Tang · Female · existing PNG companion packs. `set_companion_pack` accepts reserved ids `tang` / `female` without a pack directory. Persist `imagePackId`. Credit line when Tang is selected (jupifox CC BY).

5. Cameras: Full frames the figure, ¾ yaw, FOV ~35. Head looks at `DEF-HEAD`, FOV **28**. Corner always Head. Play clip `idle` looping. Fallback orbit only if the clip is missing. Reduce Motion pauses. In-flight send may play `think`.

6. Sprite Off: no canvas. Think-v9: 3D holds. Fail-soft to spirit + quiet Settings note. Ghost hits: canvas = floatwrap.

7. `./scripts/install-stable.sh` then git commit `-m "GO-ORBITTY-COMPANION-3D"` (include the glbs + License.txt if untracked). Do not push. STOP.

## Do not

`tauri dev`. Stamp. `companion.play`. Unity. Default 3D. Data-URL the GLB. Load three on the rail. PNG bake. VRoid. Caption leftover. Planet walkers.
