"""Project eye cards + lid bones into bust camera pixel space."""
from __future__ import annotations

import sys
from pathlib import Path

import bpy
from bpy_extras.object_utils import world_to_camera_view

sys.path.insert(0, str(Path(__file__).parent))
import sculpt_cos as S  # noqa: E402


def main():
    S.main()
    scene = bpy.context.scene
    cam = bpy.data.objects["bust"]
    body = bpy.data.objects["CoS_Body"]
    vg = {g.index: g.name for g in body.vertex_groups}
    rx, ry = scene.render.resolution_x, scene.render.resolution_y

    def proj(w):
        ndc = world_to_camera_view(scene, cam, w)
        return (round(ndc.x * rx, 1), round(ndc.y * ry, 1))

    for side in (".L", ".R"):
        cards = [body.matrix_world @ v.co for v in body.data.vertices
                 if any(vg.get(g.group) == "DEF-EYE" + side and g.weight > 0.5
                        for g in v.groups)]
        if cards:
            xs = [proj(w) for w in cards]
            px = [p[0] for p in xs]
            py = [p[1] for p in xs]
            c3 = sum(cards, cards[0].copy() * 0) / len(cards)
            print(f"EYE{side} card px x {min(px)}..{max(px)} y {min(py)}..{max(py)} "
                  f"center {proj(c3)} world_y {round(c3.y, 4)}")
    for bone in ("DEF-FACE-TOPEYELID.L", "DEF-FACE-BOTEYELID.L",
                 "DEF-FACE-TOPEYELID.R", "DEF-FACE-BOTEYELID.R"):
        pts = [body.matrix_world @ v.co for v in body.data.vertices
               if any(vg.get(g.group) == bone and g.weight > 0.5
                      for g in v.groups)]
        if pts:
            c = sum(pts, pts[0].copy() * 0) / len(pts)
            print(f"{bone} px {proj(c)} world_y {round(c.y, 4)}")


if __name__ == "__main__":
    main()
