"""GO-ORBITTY-COS-SCAFFOLD — bake 1 of 2: CoS clay from Jacob FBX.

Breaks the jupifox silhouette toward the blessed female plate
(cos_src/inspire_head.jpg): bald human head (ears/tail deleted, fur nubbed,
hair/goggles are painted texture -> skin raster), muzzle flattened to human
face planes, slim nose, head x1.65 slight-chibi, big magenta cel eyes on a
dedicated eye texture, travelogue clothes remap, collar hint. Body close.

Safety:
- NEVER opens Jacob.blend. CLI must pass -Y / --disable-autoexec.
- Imports Jacob.fbx + textures only. Rest pose. No idle frames.
"""
from __future__ import annotations

import json
import math
from pathlib import Path

import bmesh
import bpy
from mathutils import Vector, kdtree

ROOT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion")
SRC = ROOT / "jacob_src"
FBX = SRC / "Jacob.fbx"
TEX = SRC / "textures"
OUT = ROOT / "cos_src"
OUTTEX = OUT / "textures"
BAKE = ROOT / "cos_bake"
REPORT = BAKE / "sculpt_cos_report.json"
W, H = 256, 384

SKIN = (0.949, 0.784, 0.635)        # warm pale plate skin
SKIN_SHADOW = (0.851, 0.627, 0.471)
BLUSH = (0.910, 0.604, 0.627)
IRIS = (0.784, 0.314, 0.486)        # plate magenta-rose
IRIS_DARK = (0.545, 0.188, 0.353)
PUPIL = (0.180, 0.098, 0.149)
EYE_WHITE = (0.965, 0.949, 0.937)
LASH = (0.290, 0.188, 0.157)
MOUTH = (0.753, 0.439, 0.408)
OCHRE = (0.788, 0.635, 0.373)       # travelogue tunic
TEAL = (0.184, 0.420, 0.420)        # travelogue pants
UMBER = (0.353, 0.251, 0.188)       # boots
TERRA = (0.788, 0.435, 0.290)       # collar accent

DELETE_GROUPS = {
    "DEF-EAR_01.L", "DEF-EAR_02.L", "DEF-EAR_01.R", "DEF-EAR_02.R",
    "DEF-TAIL01", "DEF-TAIL02", "DEF-TAIL03", "DEF-TAIL04",
    "DEF-TAIL05", "DEF-TAIL06", "DEF-TAIL07",
    # legacy flat eye cards: tilted, buried after head scale — replaced by
    # fresh forward-facing cards built in make_eye_cards()
    "DEF-EYE.L", "DEF-EYE.R",
}
SHRINK_GROUPS = {  # fur tufts / whiskers / beard -> tiny hidden nubs
    "DEF-BEARD01": 0.22, "DEF-BEARD02": 0.22,
    "DEF-FACE-WHISKERS.L": 0.20, "DEF-FACE-WHISKERS.R": 0.20,
    "DEF-FUR-CHEEK01.L": 0.22, "DEF-FUR-CHEEK02.L": 0.22,
    "DEF-FUR-CHEEKLOW.L": 0.22, "DEF-FUR-CHEEK01.R": 0.22,
    "DEF-FUR-CHEEK02.R": 0.22, "DEF-FUR-CHEEKLOW.R": 0.22,
    "DEF-FUR-BACK01": 0.22, "DEF-FUR-BACK02": 0.22,
    "DEF-FUR-BACK03": 0.22, "DEF-FUR-BACK04": 0.22,
}
HEAD_PIVOT = Vector((0.0, 0.0, 1.50))
HEAD_SCALE = 1.65
HEAD_CENTER = Vector((0.0, 0.0, 1.662))
EYE_DROP = 0.015
DROP_MESHES = {
    "JacobMedieval-Cloak", "JacobMedieval-Pauldron", "JacobMedieval-Claymore",
    "JacobMedieval-Belt", "JacobMedieval-ShirtBonds",
    "Grass", "Ground", "Rocks",
}
RENAME = {
    "Jacob": "CoS_Body", "JacobOutline": "CoS_Outline",
    "JacobMedieval-Shirt": "CoS_Shirt", "JacobMedieval-Pants": "CoS_Pants",
    "JacobMedieval-Boots": "CoS_Boots",
}


def is_head_group(name: str) -> bool:
    return (
        name in {"DEF-HEAD", "DEF-JAW"}
        or name.startswith("DEF-FACE-")
        or name.startswith("DEF-EYE")
        or name.startswith("DEF-TEETH")
        or name.startswith("DEF-TONGUE")
    )


def is_eye_asm(name: str) -> bool:
    return name.startswith("DEF-EYE")


# ------------------------------------------------------------ mesh editing
def plan_edits(obj):
    me = obj.data
    vg = {g.index: g.name for g in obj.vertex_groups}
    shrink_centroids = {}
    for v in me.vertices:
        for g in v.groups:
            name = vg.get(g.group, "")
            if name in SHRINK_GROUPS and g.weight > 0.2:
                c = shrink_centroids.setdefault(name, [Vector((0, 0, 0)), 0])
                c[0] += v.co
                c[1] += 1
    for c in shrink_centroids.values():
        c[0] /= max(1, c[1])

    new_co, delete, anchor = {}, {}, {}
    stats = {"deleted": 0, "shrunk": 0, "muzzle": 0, "jaw": 0, "nose": 0,
             "neck": 0, "head_scaled": 0, "plate_flat": 0}
    for i, v in enumerate(me.vertices):
        p = v.co.copy()
        weights = {vg.get(g.group, ""): g.weight for g in v.groups}
        del_hit = [n for n, w in weights.items() if n in DELETE_GROUPS and w > 0.2]
        if del_hit:
            delete[i] = True
            gname = del_hit[0]
            if "EAR" in gname:
                side = ".L" if gname.endswith(".L") else ".R"
                anchor[i] = Vector((0.0551 if side == ".L" else -0.0551, 0.0258, 1.7195))
            elif "EYE" in gname:
                anchor[i] = HEAD_CENTER * 0.88  # sink inside the skull
            else:
                anchor[i] = Vector((0.0, 0.10, 1.0311))
            stats["deleted"] += 1
            continue

        head_w = max([w for n, w in weights.items()
                      if is_head_group(n) and w > 0.25] or [0.0])
        in_head = head_w > 0.0

        sh = [(SHRINK_GROUPS[n], n) for n, w in weights.items()
              if n in SHRINK_GROUPS and w > 0.2]
        if sh:
            factor, gname = max(sh)
            c = shrink_centroids[gname][0]
            p = c + (p - c) * factor
            stats["shrunk"] += 1

        if in_head:
            # muzzle flatten: snout protrusion back toward the face plane
            if p.y < -0.095:
                d = -0.095 - p.y
                p.y = -0.095 - d * 0.32
                stats["muzzle"] += 1
            # jaw narrow + chin lift (feminine lower face)
            f = min(1.0, max(0.0, (1.660 - p.z) / 0.10))
            if f > 0:
                p.x *= 1.0 - 0.16 * f
                stats["jaw"] += 1
            if p.z < 1.565:
                p.z += (1.565 - p.z) * 0.22
            # slim human nose
            nose_w = max([w for n, w in weights.items()
                          if n.startswith("DEF-FACE-NOSE")] or [0.0])
            if nose_w > 0.25:
                p.x *= 0.82
                p.z += 0.003
                stats["nose"] += 1
            # plate face: press the eye socket + brow ridge toward a flat
            # front plane so the graphic eye cards sit clean (no occlusion
            # fight). Feathered edges; nose bridge (|x|<0.012) untouched.
            if (1.615 <= p.z <= 1.725 and p.y < -0.028
                    and 0.012 <= abs(p.x) <= 0.105):
                ez = (p.z - 1.615) / 0.11
                ex = min(1.0, (abs(p.x) - 0.012) / 0.02)
                feather = max(0.0, min(1.0, ez / 0.15, (1.0 - ez) / 0.15)) * ex
                target = -0.070 + (p.y + 0.070) * 0.10
                p.y += (target - p.y) * feather
                stats["plate_flat"] += 1
            # slight-chibi head scale
            p = HEAD_PIVOT + (p - HEAD_PIVOT) * HEAD_SCALE
            stats["head_scaled"] += 1
            # eye zone (lids/corners): drop for plate-like low big eyes
            if any(is_eye_asm(n) and w > 0.25 for n, w in weights.items()):
                p.z -= EYE_DROP

        neck_w = weights.get("DEF-NECK", 0.0)
        if neck_w > 0.4 and not in_head:
            p.x *= 0.88
            p.y = 0.026 + (p.y - 0.026) * 0.88
            # blend the neck into the scaled head (no seam band)
            s_eff = 1.0 + (HEAD_SCALE - 1.0) * min(1.0, max(0.0, (p.z - 1.50) / 0.10))
            if s_eff > 1.0:
                p = HEAD_PIVOT + (p - HEAD_PIVOT) * s_eff
            stats["neck"] += 1

        if p != v.co:
            new_co[i] = p
    return new_co, delete, anchor, stats


def apply_edits(obj, new_co, delete, fill_centers, skin_uv):
    me = obj.data
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.verts.ensure_lookup_table()
    for i, p in new_co.items():
        if i not in delete:
            bm.verts[i].co = p
    if delete:
        bmesh.ops.delete(
            bm, geom=[bm.verts[i] for i in sorted(delete)], context="VERTS")
    # fill only the holes we created (ear / tail mounts); never eye holes
    for center, radius in fill_centers:
        edges = [e for e in bm.edges
                 if e.is_boundary
                 and all((v.co - center).length < radius for v in e.verts)]
        if edges:
            res = bmesh.ops.triangle_fill(bm, edges=edges, use_beautify=True)
            new_faces = []
            for val in res.values():
                if isinstance(val, (list, tuple)):
                    new_faces += [x for x in val
                                  if isinstance(x, bmesh.types.BMFace)]
            uv_l = bm.loops.layers.uv.active
            if uv_l and skin_uv:
                for f in new_faces:
                    f.smooth = True
                    for l in f.loops:
                        l[uv_l].uv = skin_uv
    bm.to_mesh(me)
    bm.free()
    me.update()
    for poly in me.polygons:
        poly.use_smooth = True


def map_outline(obj, src_co, new_co, delete, anchor):
    """Delta-map the inverted-hull shell; sink deleted parts inside skull."""
    me = obj.data
    kd = kdtree.KDTree(len(src_co))
    for i, co in enumerate(src_co):
        kd.insert(co, i)
    kd.balance()
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.verts.ensure_lookup_table()
    moved = 0
    for i, v in enumerate(me.vertices):
        _co, j, _d = kd.find(v.co)
        if j is None:
            continue
        if j in delete:
            c = anchor[j]
            bm.verts[i].co = HEAD_CENTER + (c - HEAD_CENTER) * 0.88
            moved += 1
        elif j in new_co:
            bm.verts[i].co = v.co + (new_co[j] - src_co[j])
            moved += 1
    bm.to_mesh(me)
    bm.free()
    me.update()
    for poly in me.polygons:
        poly.use_smooth = True
    return moved


# ------------------------------------------------------------- UV helpers
def group_loops(obj, names, min_w=0.25):
    me = obj.data
    vg = {g.index: g.name for g in obj.vertex_groups}
    uv_l = me.uv_layers.active
    out = []
    for poly in me.polygons:
        for li in poly.loop_indices:
            vi = me.loops[li].vertex_index
            v = me.vertices[vi]
            if any(vg.get(g.group, "") in names and g.weight >= min_w
                   for g in v.groups):
                out.append((vi, uv_l.data[li].uv.copy()))
    return out


class UvFrame:
    """Wrap-aware local frame for one UV island cluster."""

    def __init__(self, uvs):
        ref = uvs[0]
        self.pts = []
        for u in uvs:
            du = u - ref
            self.pts.append(Vector((
                ref.x + du.x - round(du.x), ref.y + du.y - round(du.y))))

    def centroid(self):
        c = sum(self.pts, Vector((0, 0))) / len(self.pts)
        return Vector((c.x % 1.0, c.y % 1.0))

    def bounds(self):
        xs = [p.x for p in self.pts]
        ys = [p.y for p in self.pts]
        return min(xs), min(ys), max(xs), max(ys)


# ----------------------------------------------------------- paint helpers
def blend(px, w, h, x, y, color, alpha):
    if 0 <= x < w and 0 <= y < h and alpha > 0:
        i = (y * w + x) * 4
        a = min(1.0, alpha)
        px[i] = px[i] * (1 - a) + color[0] * a
        px[i + 1] = px[i + 1] * (1 - a) + color[1] * a
        px[i + 2] = px[i + 2] * (1 - a) + color[2] * a
        px[i + 3] = 1.0


def disc(px, w, h, cx, cy, r, color, alpha=1.0, feather=1.5):
    for y in range(int(cy - r - 2), int(cy + r + 3)):
        for x in range(int(cx - r - 2), int(cx + r + 3)):
            d = math.hypot(x - cx, y - cy)
            if d <= r + feather:
                a = alpha * min(1.0, max(0.0, (r + feather - d) / max(feather, 1e-6)))
                blend(px, w, h, x % w, y % h, color, a)


def ellipse(px, w, h, cx, cy, rx, ry, color, alpha=1.0, feather=1.5):
    for y in range(int(cy - ry - 2), int(cy + ry + 3)):
        for x in range(int(cx - rx - 2), int(cx + rx + 3)):
            d = ((x - cx) / max(rx, 1e-6)) ** 2 + ((y - cy) / max(ry, 1e-6)) ** 2
            if d <= 1.15:
                a = alpha * min(1.0, max(0.0, (1.15 - d) / 0.3))
                blend(px, w, h, x % w, y % h, color, a)


def fill_tri(px, w, h, pts, color, alpha=1.0):
    p = [Vector((pt[0] * w, pt[1] * h)) for pt in pts]
    minx, maxx = int(min(q.x for q in p)) - 1, int(max(q.x for q in p)) + 1
    miny, maxy = int(min(q.y for q in p)) - 1, int(max(q.y for q in p)) + 1
    d = (p[1].y - p[2].y) * (p[0].x - p[2].x) + (p[2].x - p[1].x) * (p[0].y - p[2].y)
    if abs(d) < 1e-9:
        return
    for y in range(miny, maxy + 1):
        for x in range(minx, maxx + 1):
            l0 = ((p[1].y - p[2].y) * (x - p[2].x) + (p[2].x - p[1].x) * (y - p[2].y)) / d
            l1 = ((p[2].y - p[0].y) * (x - p[2].x) + (p[0].x - p[2].x) * (y - p[2].y)) / d
            l2 = 1 - l0 - l1
            if l0 >= -0.02 and l1 >= -0.02 and l2 >= -0.02:
                blend(px, w, h, x % w, y % h, color, alpha)


def lum(c):
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


# --------------------------------------------------------------- repaint
def repaint_body(obj, img):
    w, h = img.size
    px = list(img.pixels)
    report = {"size": [w, h]}

    # 1) global hue keys, luminance-preserving: scarf reds -> terra,
    #    fur/hair browns -> skin, dark navy -> ochre (red checked first)
    import colorsys
    skin_l, terra_l, ochre_l = lum(SKIN), lum(TERRA), lum(OCHRE)
    keys = {"fur": 0, "red": 0, "navy": 0}
    for i in range(0, len(px), 4):
        r, g, b = px[i], px[i + 1], px[i + 2]
        l = lum((r, g, b))
        hue = colorsys.rgb_to_hsv(r, g, b)[0] if (max(r, g, b) - min(r, g, b)) > 1e-4 else 0.5
        if r > 0.40 and r - g > 0.20 and r - b > 0.22 and hue < 0.045:
            s = l / terra_l
            px[i] = min(1, TERRA[0] * s)
            px[i + 1] = min(1, TERRA[1] * s)
            px[i + 2] = min(1, TERRA[2] * s)
            keys["red"] += 1
        elif r >= g > b and (r - b) > 0.08 and r > 0.08:
            s = min(1.3, max(0.55, l / skin_l))
            px[i] = min(1, SKIN[0] * s)
            px[i + 1] = min(1, SKIN[1] * s)
            px[i + 2] = min(1, SKIN[2] * s)
            keys["fur"] += 1
        elif b > r + 0.03 and l < 0.5:
            s = max(0.35, l / ochre_l)
            px[i] = min(1, OCHRE[0] * s)
            px[i + 1] = min(1, OCHRE[1] * s)
            px[i + 2] = min(1, OCHRE[2] * s)
            keys["navy"] += 1
    report["hue_keys"] = keys

    # 2) flat skin raster over every face/head/neck/chest-sampled triangle
    face_names = {n for n in (g.name for g in obj.vertex_groups)
                  if is_head_group(n) or n in {"DEF-NECK", "DEF-CHEST"}}
    me = obj.data
    vg = {g.index: g.name for g in obj.vertex_groups}
    uv_l = me.uv_layers.active
    tris = []
    for poly in me.polygons:
        uvs = []
        ok = False
        for li in poly.loop_indices:
            vi = me.loops[li].vertex_index
            v = me.vertices[vi]
            if any(vg.get(g.group, "") in face_names and g.weight > 0.08
                   for g in v.groups):
                ok = True
            uvs.append(uv_l.data[li].uv.copy())
        if ok and len(uvs) >= 3:
            ref = uvs[0]
            local = []
            for u in uvs:
                du = u - ref
                local.append((ref.x + du.x - round(du.x),
                              ref.y + du.y - round(du.y)))
            for k in range(1, len(local) - 1):
                tris.append((local[0], local[k], local[k + 1]))
    for t in tris:
        fill_tri(px, w, h, t, SKIN, 1.0)
    for t in tris:
        cx = sum(q[0] for q in t) / 3
        cy = sum(q[1] for q in t) / 3
        big = [((q[0] - cx) * 1.2 + cx, (q[1] - cy) * 1.2 + cy) for q in t]
        fill_tri(px, w, h, big, SKIN, 0.5)
    report["face_tris"] = len(tris)
    img.pixels = px
    img.update()
    return report



def make_eye_texture():
    """Dedicated 128px eye texture + material (iris centered, lash vignette)."""
    img = bpy.data.images.new("T_CoSEyes", width=128, height=128, alpha=False)
    w = h = 128
    px = [0.0] * (w * h * 4)
    for i in range(0, len(px), 4):
        px[i], px[i + 1], px[i + 2], px[i + 3] = *EYE_WHITE, 1.0
    cx = cy = 64.0
    # soft dark vignette at the island rim = eyeliner, orientation-safe
    for y in range(h):
        for x in range(w):
            d = math.hypot(x - cx, y - cy) / 64.0
            if d > 0.72:
                a = min(0.85, (d - 0.72) * 3.2)
                blend(px, w, h, x, y, LASH, a)
    disc(px, w, h, cx, cy, 40, IRIS_DARK, 1.0, 2.0)
    disc(px, w, h, cx, cy, 38, IRIS, 1.0, 2.0)
    # cel shade: thin dark crescent hugging the iris TOP EDGE only
    for k in range(40):
        a = math.pi * (k / 39.0)
        disc(px, w, h, cx + 41 * math.cos(a), cy + 41 * math.sin(a),
             4.5, IRIS_DARK, 0.4, 2.5)
    disc(px, w, h, cx, cy, 15, PUPIL, 1.0, 1.5)
    disc(px, w, h, cx - 11, cy + 12, 6.5, (1, 1, 1), 0.95, 1.5)
    disc(px, w, h, cx + 9, cy - 10, 3.5, (1, 1, 1), 0.8, 1.5)
    # new images sample v against flipped rows (proven by diag_eye.py):
    # flip so the lash arc lands at the TOP of the card
    row = w * 4
    flipped = [0.0] * len(px)
    for y in range(h):
        flipped[y * row:(y + 1) * row] = px[(h - 1 - y) * row:(h - y) * row]
    img.pixels = flipped
    img.update()

    mat = bpy.data.materials.new("CoS_Eyes")
    mat.use_nodes = True
    ntree = mat.node_tree
    bsdf = ntree.nodes.get("Principled BSDF")
    tex = ntree.nodes.new("ShaderNodeTexImage")
    tex.image = img
    ntree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    bsdf.inputs["Roughness"].default_value = 0.45
    if "Specular IOR Level" in bsdf.inputs:
        bsdf.inputs["Specular IOR Level"].default_value = 0.25
    return {"image": img, "material": mat}


def make_eye_cards(body, arm):
    """Fresh eye cards fanned over the eyelid ring of the edited body.

    The legacy cards were tilted and buried by the head scale, so they were
    deleted in plan_edits. The lid-ring verts (DEF-EYE-*EYELID*) trace the
    eye opening on the curved face; the card follows that curve, sits
    uniformly proud, and keeps the iris centered via angular UVs."""
    import bmesh
    mat = bpy.data.materials.get("CoS_Eyes")
    if mat is None:
        return []
    vg = {g.index: g.name for g in body.vertex_groups}
    made = []
    for side in (".L", ".R"):
        ring = []
        for v in body.data.vertices:
            w = max([g.weight for g in v.groups
                     if vg.get(g.group, "").startswith("DEF-EYE-")
                     and "EYELID" in vg.get(g.group, "")
                     and vg.get(g.group, "").endswith(side)] or [0.0])
            if w > 0.25:
                ring.append(body.matrix_world @ v.co)
        if len(ring) < 6:
            continue
        cx = sum(p.x for p in ring) / len(ring)
        cz = sum(p.z for p in ring) / len(ring)
        # plate eyes sit close together on the front face, not wrapped to
        # the temples: pull the center toward the midline, tighten the ring
        cx *= 0.68
        ring = [Vector((cx + (p.x - cx) * 0.80, p.y, cz + (p.z - cz) * 0.80))
                for p in ring]
        ring.sort(key=lambda p: math.atan2(p.z - cz, p.x - cx))
        # subsample to a clean ~14-gon, proud of the BLACK OUTLINE shell
        # (shell floats ~0.004 over the skin, so the card must clear it)
        step = max(1, len(ring) // 14)
        rim = ring[::step]
        front_y = min(p.y for p in rim)
        verts = [(cx, front_y - 0.011, cz)]  # bowed-forward center
        for p in rim:
            verts.append((p.x, p.y - 0.0075, p.z))
        n = len(rim)
        faces = [(0, 1 + k, 1 + (k + 1) % n) for k in range(n)]
        me = bpy.data.meshes.new("CoS_EyeCard" + side)
        me.from_pydata(verts, [], faces)
        me.update()
        ob = bpy.data.objects.new("CoS_EyeCard" + side, me)
        bpy.context.collection.objects.link(ob)
        me.materials.append(mat)
        uv = me.uv_layers.new(name="UVMap")
        for poly in me.polygons:
            for li in poly.loop_indices:
                vi = me.loops[li].vertex_index
                if vi == 0:
                    uv.data[li].uv = (0.5, 0.5)
                else:
                    p = rim[vi - 1]
                    a = math.atan2(p.z - cz, p.x - cx)
                    uv.data[li].uv = (0.5 + 0.46 * math.cos(a),
                                      0.5 + 0.46 * math.sin(a))
        # normals must face -Y (toward the viewer)
        bm = bmesh.new()
        bm.from_mesh(me)
        avg_y = sum(f.normal.y for f in bm.faces) / max(1, len(bm.faces))
        if avg_y > 0:
            for f in bm.faces:
                f.normal_flip()
        bm.to_mesh(me)
        bm.free()
        me.update()
        vgroup = ob.vertex_groups.new(name="DEF-EYE" + side)
        vgroup.add(list(range(len(verts))), 1.0, "REPLACE")
        ob.parent = arm
        mod = ob.modifiers.new("Armature", "ARMATURE")
        mod.object = arm
        made.append({"side": side, "center": [round(cx, 4), round(front_y, 4),
                                              round(cz, 4)], "rim": n})
    return made


def swatch_remap(obj, img, target, skip):
    me = obj.data
    uv_l = me.uv_layers.active
    w, h = img.size
    px = list(img.pixels)
    tl = lum(target)
    hits = set()
    for poly in me.polygons:
        for li in poly.loop_indices:
            u = uv_l.data[li].uv
            x = int((u.x % 1.0) * w) % w
            y = int((u.y % 1.0) * h) % h
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    hits.add(((x + dx) % w, (y + dy) % h))
    changed = 0
    for (x, y) in hits:
        if (x, y) in skip:
            continue
        i = (y * w + x) * 4
        src = (px[i], px[i + 1], px[i + 2])
        l = lum(src)
        if l < 0.03 or l > 0.95:
            continue
        s = l / tl
        px[i] = min(1, target[0] * s)
        px[i + 1] = min(1, target[1] * s)
        px[i + 2] = min(1, target[2] * s)
        skip.add((x, y))
        changed += 1
    img.pixels = px
    img.update()
    return changed


# ---------------------------------------------------------------- collar
def add_collar(arm, body):
    me = body.data
    vg = {g.index: g.name for g in body.vertex_groups}
    neck_pts = [v.co for v in me.vertices
                if any(vg.get(g.group) == "DEF-NECK" and g.weight > 0.4
                       for g in v.groups)]
    if not neck_pts:
        return None
    z_top = max(p.z for p in neck_pts) - 0.005
    z_bot = z_top - 0.085
    cy = sum(p.y for p in neck_pts) / len(neck_pts)
    r_neck = max(math.hypot(p.x, p.y - cy) for p in neck_pts)
    r1, r2 = r_neck * 1.10, r_neck * 1.62
    n = 16
    verts, faces = [], []
    for k in range(n):
        a = 2 * math.pi * k / n
        verts.append((r1 * math.cos(a), cy + r1 * math.sin(a), z_top))
        verts.append((r2 * math.cos(a), cy + r2 * math.sin(a), z_bot))
    for k in range(n):
        a, b = k * 2, ((k + 1) % n) * 2
        faces.append((a, b, b + 1, a + 1))
    me2 = bpy.data.meshes.new("CoS_Collar")
    me2.from_pydata(verts, [], faces)
    me2.update()
    for poly in me2.polygons:
        poly.use_smooth = True
    ob = bpy.data.objects.new("CoS_Collar", me2)
    bpy.context.collection.objects.link(ob)
    ob.parent = arm
    gi = ob.vertex_groups.new(name="DEF-NECK")
    gi.add(list(range(len(verts))), 1.0, "REPLACE")
    mod = ob.modifiers.new("Armature", "ARMATURE")
    mod.object = arm
    mat = bpy.data.materials.new("CoS_Collar")
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = (*TERRA, 1.0)
    bsdf.inputs["Roughness"].default_value = 0.7
    me2.materials.append(mat)
    return {"z_top": round(z_top, 4), "z_bot": round(z_bot, 4),
            "r1": round(r1, 4), "r2": round(r2, 4)}


# ---------------------------------------------------------------- render
def setup_render():
    s = bpy.context.scene
    try:
        s.render.engine = "BLENDER_EEVEE_NEXT"
    except Exception:
        s.render.engine = "CYCLES"
        s.cycles.samples = 24
    s.render.resolution_x = W
    s.render.resolution_y = H
    s.render.resolution_percentage = 100
    s.render.film_transparent = True
    s.render.image_settings.file_format = "PNG"
    s.render.image_settings.color_mode = "RGBA"
    s.render.image_settings.color_depth = "8"
    try:
        s.view_settings.view_transform = "Standard"
        s.view_settings.look = "None"
        s.view_settings.exposure = 0.15
        s.view_settings.gamma = 1.0
    except Exception:
        pass
    if not s.world:
        s.world = bpy.data.worlds.new("W")
    s.world.use_nodes = True
    bg = s.world.node_tree.nodes.get("Background")
    if bg:
        bg.inputs[0].default_value = (0.55, 0.64, 0.74, 1)
        bg.inputs[1].default_value = 0.28
    for name, loc, energy, size in [
        ("Key", (2.2, -3.4, 3.6), 380, 3.2),
        ("Fill", (-3.0, -1.4, 2.2), 120, 4.5),
        ("Rim", (0.4, 3.6, 2.8), 160, 2.8),
        ("Front", (0.0, -4.4, 1.8), 140, 3.8),
    ]:
        ld = bpy.data.lights.new(name, type="AREA")
        ld.energy = energy
        ld.size = size
        o = bpy.data.objects.new(name, ld)
        bpy.context.collection.objects.link(o)
        o.location = loc
        o.rotation_euler = (
            math.radians(50), 0,
            math.atan2(loc[0], -loc[1]) if loc[1] else 0)


def char_bounds():
    mins = Vector((1e9, 1e9, 1e9))
    maxs = Vector((-1e9, -1e9, -1e9))
    deps = bpy.context.evaluated_depsgraph_get()
    for o in bpy.data.objects:
        if o.type != "MESH" or o.hide_render:
            continue
        ev = o.evaluated_get(deps)
        m = ev.to_mesh()
        try:
            for v in m.vertices:
                wv = ev.matrix_world @ v.co
                mins.x, mins.y, mins.z = min(mins.x, wv.x), min(mins.y, wv.y), min(mins.z, wv.z)
                maxs.x, maxs.y, maxs.z = max(maxs.x, wv.x), max(maxs.y, wv.y), max(maxs.z, wv.z)
        finally:
            ev.to_mesh_clear()
    return mins, maxs


def make_camera(name, look, dist, azimuth_deg, elev_frac=0.0):
    cam_d = bpy.data.cameras.new(name)
    cam_d.lens = 70
    cam_d.clip_start = 0.01
    cam_d.clip_end = 200
    cam = bpy.data.objects.new(name, cam_d)
    bpy.context.collection.objects.link(cam)
    a = math.radians(azimuth_deg)
    cam.location = Vector((
        look.x + dist * math.sin(a),
        look.y - dist * math.cos(a),
        look.z + dist * elev_frac))
    direction = look - cam.location
    cam.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()
    return cam


def render_shot(cam, name):
    bpy.context.scene.camera = cam
    out = OUT / f"{name}.png"
    bpy.context.scene.render.filepath = str(out.with_suffix(""))
    bpy.ops.render.render(write_still=True)
    if not out.exists():
        cands = list(OUT.glob(f"{name}*.png"))
        if cands:
            cands[0].replace(out)
    return str(out), out.exists()


# ------------------------------------------------------- projection paint
def _cam_ray(cam, scene, x, y):
    rx = scene.render.resolution_x
    ry = scene.render.resolution_y
    u = (x + 0.5) / rx
    v = (y + 0.5) / ry
    tr, br, bl, tl = cam.data.view_frame(scene=scene)
    point = bl * (1 - u) * (1 - v) + br * u * (1 - v) + tl * (1 - u) * v + tr * u * v
    direction = (cam.matrix_world.to_quaternion() @ point).normalized()
    return cam.matrix_world.translation, direction


def _poly_uv(obj, poly_index, hit_loc):
    from mathutils.geometry import tessellate_polygon
    me = obj.data
    uv_l = me.uv_layers.active
    if uv_l is None:
        return None
    poly = me.polygons[poly_index]
    local = obj.matrix_world.inverted() @ hit_loc
    verts = [me.vertices[vi].co for vi in poly.vertices]
    uvs = [uv_l.data[li].uv for li in poly.loop_indices]
    for tri in tessellate_polygon([verts]):
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
    return None


def projection_paint(body, img, cam, features, region):
    """Paint features defined in bust-render pixel space (bottom-up origin)
    onto the exact texels visible there. Marches through shell occluders."""
    scene = bpy.context.scene
    deps = bpy.context.evaluated_depsgraph_get()
    w, h = img.size
    px = list(img.pixels)
    x0r, y0r, x1r, y1r = region
    texel_hits = {}
    for y in range(y0r, y1r):
        for x in range(x0r, x1r):
            origin, direction = _cam_ray(cam, scene, x, y)
            for _bounce in range(4):
                hit, loc, _n, poly_index, obj, _m = scene.ray_cast(
                    deps, origin, direction)
                if not hit or obj is None:
                    break
                if obj is body:
                    uv = _poly_uv(obj, poly_index, loc)
                    if uv:
                        tx = int(uv[0] * w) % w
                        ty = int(uv[1] * h) % h
                        texel_hits.setdefault((tx, ty), []).append((x, y))
                    break
                origin = loc + direction * 0.002  # step through the shell
    painted = 0
    for (cx, cy, rx, ry, color, alpha) in features:
        for (tx, ty), pixels in texel_hits.items():
            mx = sum(p[0] for p in pixels) / len(pixels)
            my = sum(p[1] for p in pixels) / len(pixels)
            d = ((mx - cx) / max(rx, 1e-6)) ** 2 + ((my - cy) / max(ry, 1e-6)) ** 2
            if d <= 1.0:
                blend(px, w, h, tx, ty, color, alpha * (1.0 - 0.55 * d))
                painted += 1
    img.pixels = px
    img.update()
    return {"texels_seen": len(texel_hits), "texels_painted": painted}


# ------------------------------------------------------------------ main
def main():
    OUT.mkdir(parents=True, exist_ok=True)
    OUTTEX.mkdir(parents=True, exist_ok=True)
    rep = {}
    bpy.ops.wm.read_factory_settings(use_empty=True)
    setup_render()
    bpy.ops.import_scene.fbx(
        filepath=str(FBX), automatic_bone_orientation=False,
        use_anim=True, ignore_leaf_bones=False, anim_offset=0.0)

    for o in list(bpy.data.objects):
        if o.name in DROP_MESHES:
            bpy.data.objects.remove(o, do_unlink=True)
    rep["dropped"] = sorted(DROP_MESHES)

    body = bpy.data.objects["Jacob"]
    outline = bpy.data.objects["JacobOutline"]
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    arm.data.pose_position = "REST"  # rest pose for inspect stills

    color_img = bpy.data.images.load(str(TEX / "T_JacobColor.png"), check_existing=True)
    props_img = bpy.data.images.load(str(TEX / "T_PropsAtlas.png"), check_existing=True)
    for mat in bpy.data.materials:
        if not mat.use_nodes:
            continue
        ntree = mat.node_tree
        lname = mat.name.lower()
        if "scene" in lname:
            continue
        target = props_img if ("prop" in lname or "atlas" in lname) else color_img
        tex = next((n for n in ntree.nodes if n.type == "TEX_IMAGE"), None)
        if tex is None:
            tex = ntree.nodes.new("ShaderNodeTexImage")
        tex.image = target
        bsdf = ntree.nodes.get("Principled BSDF")
        if bsdf:
            if not any(l.to_node == bsdf and l.to_socket.name == "Base Color"
                       for l in ntree.links):
                ntree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
            if "Specular IOR Level" in bsdf.inputs:
                bsdf.inputs["Specular IOR Level"].default_value = 0.12
            if "Roughness" in bsdf.inputs:
                bsdf.inputs["Roughness"].default_value = 0.62
    rep["texture_sizes"] = {"color": list(color_img.size), "props": list(props_img.size)}

    # ---- sculpt
    src_co = [v.co.copy() for v in body.data.vertices]
    new_co, delete, anchor, stats = plan_edits(body)
    rep["edit_stats"] = stats

    cheek = group_loops(body, {"DEF-FACE-CHEEK.L", "DEF-FACE-CHEEK.R"}, 0.3)
    skin_uv = None
    if cheek:
        c = UvFrame([u for _v, u in cheek]).centroid()
        skin_uv = (c.x, c.y)

    ear_l = HEAD_PIVOT + (Vector((0.0551, 0.0258, 1.7195)) - HEAD_PIVOT) * HEAD_SCALE
    ear_r = HEAD_PIVOT + (Vector((-0.0551, 0.0258, 1.7195)) - HEAD_PIVOT) * HEAD_SCALE
    tail = Vector((0.0, 0.10, 1.0311))
    apply_edits(body, new_co, delete,
                [(ear_l, 0.11), (ear_r, 0.11), (tail, 0.12)], skin_uv)
    rep["outline_mapped"] = map_outline(outline, src_co, new_co, delete, anchor)
    rep["collar"] = add_collar(arm, body)

    for old, new in RENAME.items():
        o = bpy.data.objects.get(old)
        if o:
            o.name = new
    m = bpy.data.materials.get("Jacob_Body")
    if m:
        m.name = "CoS_Body"
    m = bpy.data.materials.get("Jacob_PropsAtlas")
    if m:
        m.name = "CoS_PropsAtlas"

    # ---- cameras first (projection paint needs the bust cam)
    mins, maxs = char_bounds()
    height = maxs.z - mins.z
    center = (mins + maxs) * 0.5
    rep["bounds"] = {"min": [round(v, 3) for v in mins],
                     "max": [round(v, 3) for v in maxs],
                     "height": round(height, 3)}
    look_full = Vector((center.x, center.y, mins.z + height * 0.48))
    dist = max(height * 2.55, (maxs.x - mins.x) * 3.15)
    cam_front = make_camera("front", look_full, dist, 0.0, 0.06)
    cam_tq = make_camera("three_quarter", look_full, dist, 38.0, 0.06)
    region = height * 0.27
    look_bust = Vector((center.x, center.y, maxs.z - region * 0.58))
    cam_bust = make_camera("bust", look_bust, region * 2.7, 16.0, 0.05)

    # ---- repaint
    body_obj = bpy.data.objects["CoS_Body"]
    rep["body_paint"] = repaint_body(body_obj, color_img)
    eye_tex = make_eye_texture()
    eye_img = eye_tex["image"]
    arm_obj = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    rep["eye_cards"] = make_eye_cards(body_obj, arm_obj)
    skip = set()
    rep["swatch_pants"] = swatch_remap(bpy.data.objects["CoS_Pants"], props_img, TEAL, skip)
    rep["swatch_shirt"] = swatch_remap(bpy.data.objects["CoS_Shirt"], props_img, OCHRE, skip)
    rep["swatch_boots"] = swatch_remap(bpy.data.objects["CoS_Boots"], props_img, UMBER, skip)

    # ---- projection paint: features placed in bust pixel space (bottom-up).
    # Coords measured from landmark projection (lips top-down (88,160-173),
    # nose bottom (85,137), cheek.L (175,153); bottom-up y = 384 - y_top).
    rep["projection"] = projection_paint(
        body_obj, color_img, cam_bust,
        features=[
            (95, 206, 22, 16, SKIN, 1.0),       # muzzle/chin cleanup
            (124, 283, 34, 10, SKIN, 0.85),     # hairline band cleanup
            (88, 217, 9, 3.4, MOUTH, 1.0),      # small rose mouth
            (88, 218, 6.5, 1.2, (0.42, 0.22, 0.22), 0.7),
            (172, 226, 9, 7, BLUSH, 0.18),
            (68, 226, 9, 7, BLUSH, 0.18),
        ],
        region=(40, 90, 220, 300))

    color_img.filepath_raw = str(OUTTEX / "T_CoSColor.png")
    color_img.file_format = "PNG"
    color_img.save()
    color_img.name = "T_CoSColor"
    props_img.filepath_raw = str(OUTTEX / "T_CoSProps.png")
    props_img.file_format = "PNG"
    props_img.save()
    props_img.name = "T_CoSProps"
    if eye_img:
        eye_img.filepath_raw = str(OUTTEX / "T_CoSEyes.png")
        eye_img.file_format = "PNG"
        eye_img.save()

    # ---- inspect stills (rest pose; no idle)
    shots = {}
    shots["front"] = render_shot(cam_front, "front")
    shots["three_quarter"] = render_shot(cam_tq, "three_quarter")
    shots["bust"] = render_shot(cam_bust, "bust")
    rep["shots"] = shots

    # ---- export our FBX
    fbx_out = OUT / "CoS.fbx"
    kwargs = dict(
        filepath=str(fbx_out), use_selection=False,
        object_types={"ARMATURE", "MESH"}, apply_scale_options="FBX_SCALE_NONE",
        bake_anim=True, add_leaf_bones=False, path_mode="AUTO",
        embed_textures=False, use_mesh_modifiers=True,
        mesh_smooth_type="FACE", use_triangles=False)
    try:
        bpy.ops.export_scene.fbx(bake_anim_use_all_actions=True, **kwargs)
    except TypeError:
        bpy.ops.export_scene.fbx(**kwargs)
    rep["fbx"] = str(fbx_out)
    rep["fbx_bytes"] = fbx_out.stat().st_size if fbx_out.exists() else 0

    REPORT.write_text(json.dumps(rep, indent=2, default=str))
    print("SCULPT_OK", json.dumps(stats))


if __name__ == "__main__":
    main()
