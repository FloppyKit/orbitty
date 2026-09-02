"""GO-ORBITTY-COS-SCAFFOLD — inspect Jacob.fbx structure for the clay reshape.

Read-only probe: dumps meshes, vertex groups, materials, UV bounds, bone list.
NEVER opens Jacob.blend. CLI must pass -Y / --disable-autoexec.
"""
from __future__ import annotations

import json
from pathlib import Path

import bpy
from mathutils import Vector

SRC = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion/jacob_src")
FBX = SRC / "Jacob.fbx"
REPORT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion/cos_bake/inspect_cos.json")


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(
        filepath=str(FBX),
        automatic_bone_orientation=False,
        use_anim=False,
        ignore_leaf_bones=False,
    )

    info = {"objects": [], "bones": [], "images": [], "materials": []}
    for o in bpy.data.objects:
        entry = {
            "name": o.name,
            "type": o.type,
            "parent": o.parent.name if o.parent else None,
            "loc": [round(v, 4) for v in o.location],
            "scale": [round(v, 4) for v in o.scale],
        }
        if o.type == "MESH":
            me = o.data
            entry["verts"] = len(me.vertices)
            entry["faces"] = len(me.polygons)
            entry["materials"] = [m.name if m else None for m in me.materials]
            entry["uv_maps"] = [u.name for u in me.uv_layers]
            entry["vertex_groups"] = [g.name for g in o.vertex_groups]
            entry["modifiers"] = [m.type for m in o.modifiers]
            ws = [o.matrix_world @ v.co for v in me.vertices]
            if ws:
                mins = Vector((min(v.x for v in ws), min(v.y for v in ws), min(v.z for v in ws)))
                maxs = Vector((max(v.x for v in ws), max(v.y for v in ws), max(v.z for v in ws)))
                entry["world_min"] = [round(v, 4) for v in mins]
                entry["world_max"] = [round(v, 4) for v in maxs]
            # UV bounds per mesh (first UV layer)
            if me.uv_layers:
                uvs = [l.uv for l in me.uv_layers[0].data]
                if uvs:
                    entry["uv_min"] = [round(min(u.x for u in uvs), 4), round(min(u.y for u in uvs), 4)]
                    entry["uv_max"] = [round(max(u.x for u in uvs), 4), round(max(u.y for u in uvs), 4)]
            # per-vertex-group population (top slice only, for head-bone discovery)
            vg_pop = {}
            for v in me.vertices:
                for g in v.groups:
                    name = o.vertex_groups[g.group].name if g.group < len(o.vertex_groups) else str(g.group)
                    if g.weight > 0.3:
                        vg_pop[name] = vg_pop.get(name, 0) + 1
            entry["vg_population"] = dict(sorted(vg_pop.items(), key=lambda kv: -kv[1])[:24])
        info["objects"].append(entry)

    arm = next((o for o in bpy.data.objects if o.type == "ARMATURE"), None)
    if arm:
        for b in arm.data.bones:
            info["bones"].append({
                "name": b.name,
                "head_world": [round(v, 4) for v in (arm.matrix_world @ b.head_local)],
                "parent": b.parent.name if b.parent else None,
            })
    info["images"] = [{"name": i.name, "size": list(i.size), "filepath": i.filepath} for i in bpy.data.images]
    info["materials"] = [m.name for m in bpy.data.materials]
    REPORT.write_text(json.dumps(info, indent=2))
    print("INSPECT_OK", len(info["objects"]), "objects", len(info["bones"]), "bones")


if __name__ == "__main__":
    main()
