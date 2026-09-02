"""Diagnostic: emission eye material + test pattern, re-render bust only."""
from __future__ import annotations

import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).parent))
import sculpt_cos as S  # noqa: E402


def main():
    S.main()

    # test pattern: quadrants red/green/blue/yellow with white cross
    img = bpy.data.images["T_CoSEyes"]
    w = h = 128
    px = list(img.pixels)
    for y in range(h):
        for x in range(w):
            i = (y * w + x) * 4
            if abs(x - 64) < 3 or abs(y - 64) < 3:
                c = (1, 1, 1)
            elif x < 64 and y < 64:
                c = (1, 0, 0)
            elif x >= 64 and y < 64:
                c = (0, 1, 0)
            elif x < 64 and y >= 64:
                c = (0, 0, 1)
            else:
                c = (1, 1, 0)
            px[i], px[i + 1], px[i + 2], px[i + 3] = *c, 1.0
    img.pixels = px
    img.update()

    # emission material
    mat = bpy.data.materials["CoS_Eyes"]
    nt = mat.node_tree
    nt.nodes.clear()
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    em = nt.nodes.new("ShaderNodeEmission")
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    nt.links.new(tex.outputs["Color"], em.inputs["Color"])
    nt.links.new(em.outputs["Emission"], out.inputs["Surface"])

    cam = bpy.data.objects["bust"]
    bpy.context.scene.camera = cam
    bpy.context.scene.render.filepath = str(
        Path(__file__).parent / "diag_eye_emission")
    bpy.ops.render.render(write_still=True)
    print("DIAG_DONE")


if __name__ == "__main__":
    main()
