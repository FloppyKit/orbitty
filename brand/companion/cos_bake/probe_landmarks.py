"""Project face landmarks into bust pixel space (top-down coords)."""
from __future__ import annotations

import sys
from pathlib import Path

import bpy
from bpy_extras.object_utils import world_to_camera_view

sys.path.insert(0, str(Path(__file__).parent))
import sculpt_cos as S  # noqa: E402

LANDMARKS = [
    "DEF-FACE-NOSEBRIDGE", "DEF-FACE-NOSEBOT", "DEF-FACE-NOSETIP",
    "DEF-FACE-LIPTOPCENTER", "DEF-FACE-LIPBOTCENTER",
    "DEF-FACE-MOUTHUPPER", "DEF-FACE-MOUTHLOWER",
    "DEF-FACE-CHIN", "DEF-FACE-JAW",
    "DEF-FACE-EYEBROW2.L", "DEF-FACE-EYEBROW2.R",
    "DEF-FACE-TOPEYELID.L", "DEF-FACE-BOTEYELID.L",
    "DEF-FACE-CHEEK.L", "DEF-FACE-CHEEKBONE.L",
    "DEF-FACE-FOREHEAD", "DEF-FACE-FOREHEAD.L",
    "DEF-HEAD", "DEF-NECK",
]


def main():
    S.main()
    scene = bpy.context.scene
    cam = bpy.data.objects["bust"]
    body = bpy.data.objects["CoS_Body"]
    vg = {g.index: g.name for g in body.vertex_groups}
    rx, ry = scene.render.resolution_x, scene.render.resolution_y

    def proj(w):
        ndc = world_to_camera_view(scene, cam, w)
        return (round(ndc.x * rx, 1), round(ry - ndc.y * ry, 1))  # top-down y

    for name in LANDMARKS:
        pts = [body.matrix_world @ v.co for v in body.data.vertices
               if any(vg.get(g.group) == name and g.weight > 0.4
                      for g in v.groups)]
        if pts:
            c = sum(pts, pts[0].copy() * 0) / len(pts)
            print(f"{name:32s} px(top-down) {proj(c)}  world z {round(c.z, 3)}")

    # head top / chin extents on screen
    head_pts = [body.matrix_world @ v.co for v in body.data.vertices
                if any(vg.get(g.group) == "DEF-HEAD" and g.weight > 0.5
                      for g in v.groups)]
    ys = [proj(w)[1] for w in head_pts]
    xs = [proj(w)[0] for w in head_pts]
    print(f"HEAD screen y {min(ys)}..{max(ys)}  x {min(xs)}..{max(xs)}")


if __name__ == "__main__":
    main()
