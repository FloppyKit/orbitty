"""Raycast probe: rebuild the sculpt scene, then ask what each face pixel hits.

Answers: are eyeballs visible through the lid openings? Which texture texels
back the visible eye slits / mouth smudge / forehead band?
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import bpy
from mathutils import Vector
from mathutils.geometry import tessellate_polygon

sys.path.insert(0, str(Path(__file__).parent))
import sculpt_cos as S  # noqa: E402

REPORT = S.BAKE / "raycast_report.json"


def cam_ray(cam, scene, x, y):
    rx = scene.render.resolution_x
    ry = scene.render.resolution_y
    u = (x + 0.5) / rx
    v = (y + 0.5) / ry
    tr, br, bl, tl = cam.data.view_frame(scene=scene)
    point = bl * (1 - u) * (1 - v) + br * u * (1 - v) + tl * (1 - u) * v + tr * u * v
    direction = (cam.matrix_world.to_quaternion() @ point).normalized()
    return cam.matrix_world.translation, direction


def poly_uv(obj, poly_index, hit_loc):
    """Barycentric UV of a hit point on (possibly quad) polygon."""
    me = obj.data
    uv_l = me.uv_layers.active
    if uv_l is None:
        return None
    poly = me.polygons[poly_index]
    local = obj.matrix_world.inverted() @ hit_loc
    verts = [me.vertices[vi].co for vi in poly.vertices]
    uvs = [uv_l.data[li].uv for li in poly.loop_indices]
    tris = tessellate_polygon([verts])
    for tri in tris:
        a, b, c = (verts[tri[0]], verts[tri[1]], verts[tri[2]])
        v0, v1, v2 = b - a, c - a, local - a
        d00, d01 = v0.dot(v0), v0.dot(v1)
        d11, d20, d21 = v1.dot(v1), v2.dot(v0), v2.dot(v1)
        den = d00 * d11 - d01 * d01
        if abs(den) < 1e-12:
            continue
        bv = (d11 * d20 - d01 * d21) / den
        bw = (d00 * d21 - d01 * d20) / den
        bu = 1 - bv - bw
        if bu >= -0.05 and bv >= -0.05 and bw >= -0.05:
            uv = uvs[tri[0]] * bu + uvs[tri[1]] * bv + uvs[tri[2]] * bw
            return (uv.x % 1.0, uv.y % 1.0)
    c = sum(uvs, Vector((0, 0))) / len(uvs)
    return (c.x % 1.0, c.y % 1.0)


def main():
    S.main()
    scene = bpy.context.scene
    cam = bpy.data.objects["bust"]
    # hide the enclosing hull + collar so rays reach the body/eyeballs
    for name in ("CoS_Outline", "CoS_Collar"):
        o = bpy.data.objects.get(name)
        if o:
            o.hide_set(True)
            o.hide_render = True
    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()

    hits = []  # (x, y, obj, mat, uv)
    for y in range(95, 265, 2):
        for x in range(30, 226, 2):
            origin, direction = cam_ray(cam, scene, x, y)
            hit, loc, _n, poly_index, obj, _m = scene.ray_cast(deps, origin, direction)
            if not hit or obj is None or obj.type != "MESH":
                continue
            mat = None
            if 0 <= poly_index < len(obj.data.polygons):
                mi = obj.data.polygons[poly_index].material_index
                if mi < len(obj.data.materials) and obj.data.materials[mi]:
                    mat = obj.data.materials[mi].name
            uv = poly_uv(obj, poly_index, loc)
            hits.append({"x": x, "y": y, "obj": obj.name, "mat": mat,
                         "uv": [round(uv[0], 4), round(uv[1], 4)] if uv else None,
                         "loc": [round(v, 4) for v in loc]})

    by_mat = {}
    for hrec in hits:
        by_mat.setdefault(hrec["mat"], 0)
        by_mat[hrec["mat"]] += 1

    # fine eye-band map: one char per pixel column, rows y=135..175
    band = []
    for y in range(135, 176, 2):
        row = ""
        for x in range(55, 215, 2):
            origin, direction = cam_ray(cam, scene, x, y)
            hit, loc, _n, poly_index, obj, _m = scene.ray_cast(deps, origin, direction)
            ch = "."
            if hit and obj is not None and obj.type == "MESH":
                mi = obj.data.polygons[poly_index].material_index
                mat = obj.data.materials[mi].name if mi < len(obj.data.materials) else "?"
                ch = {"CoS_Body": "B", "CoS_Eyes": "E", "Outline": "O"}.get(mat, "?")
            row += ch
        band.append(f"{y:3d} {row}")

    # eye-band scan: which pixels see the eye material?
    eye_hits = [hrec for hrec in hits if hrec["mat"] == "CoS_Eyes"]
    # cluster eye hits L/R by x
    eye_l = [hrec for hrec in eye_hits if hrec["x"] < 128]
    eye_r = [hrec for hrec in eye_hits if hrec["x"] >= 128]

    def box(rows):
        if not rows:
            return None
        return {"x": [min(r["x"] for r in rows), max(r["x"] for r in rows)],
                "y": [min(r["y"] for r in rows), max(r["y"] for r in rows)],
                "n": len(rows)}

    REPORT.write_text(json.dumps({
        "total_hits": len(hits),
        "by_material": by_mat,
        "eye_hits": {"L": box(eye_l), "R": box(eye_r)},
        "eye_band": band,
        "hits": hits,
    }, indent=2))
    print("RAYCAST_OK", len(hits), "hits", by_mat, "eyes", box(eye_l), box(eye_r))
    for row in band:
        print(row)


if __name__ == "__main__":
    main()
