"""Focused eye probe: does the outline shell carry eye cards? What's at the slits?"""
from __future__ import annotations

import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).parent))
import sculpt_cos as S  # noqa: E402


def main():
    S.main()
    scene = bpy.context.scene

    # 1) does the outline shell have eye-card verts / non-outline faces?
    outline = bpy.data.objects["CoS_Outline"]
    vg = {g.index: g.name for g in outline.vertex_groups}
    eye_verts = [v.index for v in outline.data.vertices
                 if any(vg.get(g.group, "").startswith("DEF-EYE.") and g.weight > 0.5
                        for g in v.groups)]
    mat_use = {}
    for p in outline.data.polygons:
        mat_use[p.material_index] = mat_use.get(p.material_index, 0) + 1
    slots = [m.name if m else None for m in outline.data.materials]
    print("OUTLINE eye_verts:", len(eye_verts), "slots:", slots, "face_mat_use:", mat_use)

    body = bpy.data.objects["CoS_Body"]
    mat_use_b = {}
    for p in body.data.polygons:
        mat_use_b[p.material_index] = mat_use_b.get(p.material_index, 0) + 1
    print("BODY slots:", [m.name if m else None for m in body.data.materials],
          "face_mat_use:", mat_use_b)

    # 2) material map across the REAL eye band (bottom-up y 205..255)
    cam = bpy.data.objects["bust"]
    deps = bpy.context.evaluated_depsgraph_get()
    for y in range(205, 256, 3):
        row = ""
        for x in range(70, 195, 2):
            origin, direction = S._cam_ray(cam, scene, x, y)
            ch = "."
            for _bounce in range(5):
                hit, loc, _n, pi, obj, _m = scene.ray_cast(deps, origin, direction)
                if not hit or obj is None:
                    break
                mi = obj.data.polygons[pi].material_index
                mat = obj.data.materials[mi].name if mi < len(obj.data.materials) else "?"
                if obj.name == "CoS_Outline" and mat == "Outline":
                    origin = loc + direction * 0.003
                    continue
                ch = {"CoS_Body": "B", "CoS_Eyes": "E", "Outline": "O",
                      "CoS_Collar": "C"}.get(mat, "?")
                break
            row += ch
        print(f"{y:3d} {row}")


if __name__ == "__main__":
    main()
