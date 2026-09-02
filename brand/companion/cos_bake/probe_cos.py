"""Probe: dump head-band verts, UV islands, palette colors for sculpt tuning."""
from __future__ import annotations

import json
from pathlib import Path

import bpy

SRC = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion/jacob_src")
REPORT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion/cos_bake/probe_report.json")


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(
        filepath=str(SRC / "Jacob.fbx"), automatic_bone_orientation=False,
        use_anim=False, ignore_leaf_bones=False)
    body = bpy.data.objects["Jacob"]
    me = body.data
    vg = {g.index: g.name for g in body.vertex_groups}
    uv_l = me.uv_layers.active

    # head-band verts (z > 1.60), grouped summary + full dump of high band
    high = []
    for i, v in enumerate(me.vertices):
        if v.co.z > 1.60:
            groups = {vg.get(g.group, ""): round(g.weight, 2) for g in v.groups if g.weight > 0.2}
            high.append({
                "i": i,
                "co": [round(c, 4) for c in v.co],
                "groups": groups,
            })
    # UV per vertex for the high band
    uv_by_vert = {}
    for poly in me.polygons:
        for li in poly.loop_indices:
            vi = me.loops[li].vertex_index
            if vi not in uv_by_vert:
                uv_by_vert[vi] = uv_l.data[li].uv.copy()
    for e in high:
        u = uv_by_vert.get(e["i"])
        e["uv"] = [round(u.x % 1.0, 4), round(u.y % 1.0, 4)] if u else None

    # face-group UV bounds (wrapped)
    def uv_bounds(names, min_w=0.25):
        us = []
        for poly in me.polygons:
            for li in poly.loop_indices:
                vi = me.loops[li].vertex_index
                v = me.vertices[vi]
                if any(vg.get(g.group, "") in names and g.weight >= min_w for g in v.groups):
                    u = uv_l.data[li].uv
                    us.append((u.x % 1.0, u.y % 1.0))
        if not us:
            return None
        xs = [p[0] for p in us]
        ys = [p[1] for p in us]
        return {"n": len(us), "u": [round(min(xs), 3), round(max(xs), 3)],
                "v": [round(min(ys), 3), round(max(ys), 3)]}

    all_groups = {g.name for g in body.vertex_groups}
    islands = {
        "head_all": uv_bounds({n for n in all_groups if n == "DEF-HEAD"}),
        "face_features": uv_bounds({n for n in all_groups if n.startswith("DEF-FACE-")}),
        "eyelids": uv_bounds({n for n in all_groups if "EYELID" in n}),
        "eyeball_L": uv_bounds({"DEF-EYE.L"}, 0.5),
        "eyeball_R": uv_bounds({"DEF-EYE.R"}, 0.5),
        "lips": uv_bounds({n for n in all_groups if "LIP" in n}),
        "nose": uv_bounds({n for n in all_groups if "NOSE" in n}),
        "brows": uv_bounds({n for n in all_groups if "EYEBROW" in n}),
        "cheeks": uv_bounds({n for n in all_groups if "CHEEK" in n and "FUR" not in n}),
        "neck": uv_bounds({"DEF-NECK"}),
        "jaw": uv_bounds({"DEF-JAW"}),
        "ears": uv_bounds({n for n in all_groups if "EAR" in n}),
        "beard": uv_bounds({n for n in all_groups if "BEARD" in n}),
    }

    # palette textures
    texs = {}
    for name in ("T_JacobColor", "T_PropsAtlas"):
        p = SRC / "textures" / f"{name}.png"
        img = bpy.data.images.load(str(p), check_existing=True)
        w, h = img.size
        px = list(img.pixels)
        colors = {}
        for i in range(0, len(px), 4):
            key = (round(px[i], 2), round(px[i + 1], 2), round(px[i + 2], 2))
            colors[key] = colors.get(key, 0) + 1
        top = sorted(colors.items(), key=lambda kv: -kv[1])[:24]
        texs[name] = {"size": [w, h], "unique": len(colors),
                      "top": [{"rgb": list(k), "count": c} for k, c in top]}

    REPORT.write_text(json.dumps({
        "high_band_count": len(high), "high_band": high,
        "islands": islands, "textures": texs,
    }, indent=2))
    print("PROBE_OK", len(high), "high-band verts")


if __name__ == "__main__":
    main()
