"""Measure how deep the eye cards sit behind the face skin, per side."""
from __future__ import annotations

import sys
from pathlib import Path

import bpy
from mathutils import Vector

sys.path.insert(0, str(Path(__file__).parent))
import sculpt_cos as S  # noqa: E402


def main():
    S.main()
    scene = bpy.context.scene
    deps = bpy.context.evaluated_depsgraph_get()
    body = bpy.data.objects["CoS_Body"]
    vg = {g.index: g.name for g in body.vertex_groups}

    for side in (".L", ".R"):
        cards = [v for v in body.data.vertices
                 if any(vg.get(g.group) == "DEF-EYE" + side and g.weight > 0.5
                        for g in v.groups)]
        if not cards:
            continue
        burials = []
        for v in cards:
            w = body.matrix_world @ v.co
            origin = Vector((w.x, w.y - 0.05, w.z))
            hit, loc, _n, pi, obj, _m = scene.ray_cast(
                deps, origin, Vector((0, 1, 0)))
            if hit and obj is body:
                mi = body.data.polygons[pi].material_index
                mat = body.data.materials[mi].name
                burials.append((round(w.y - loc.y, 5), mat))
        pos = [b for b, m in burials if b > 0]
        print(f"side {side}: card verts {len(cards)}, "
              f"buried(+)=skin-in-front: {len(pos)}, "
              f"max burial {max(pos) if pos else 0}, "
              f"sample {burials[:6]}")


if __name__ == "__main__":
    main()
