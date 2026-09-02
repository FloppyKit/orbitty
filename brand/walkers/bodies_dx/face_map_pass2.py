"""
Face-map pass 2 — 🎨 Bot · pixel-artist
Primary: manual UV face island (front-facing polys → packed to plate)
Backup: face-card decal parented to head
"""
from __future__ import annotations

import bmesh
import math
from pathlib import Path

import bpy
from mathutils import Vector, Matrix

ROOT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/walkers/bodies_dx")
OUT = ROOT / "face_map_pass2"
TURNS = OUT / "turns"
CARD = OUT / "face_card"
OUT.mkdir(exist_ok=True)
TURNS.mkdir(exist_ok=True)
CARD.mkdir(exist_ok=True)

FACE = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/walkers/out/paperdoll_v0/ship_v0/heads/head_a_human_androgynous_v0.jpg")
BLEND = ROOT / "exports" / "femhumandx_waist_cinch.blend"


def setup_render():
    s = bpy.context.scene
    try:
        s.render.engine = "BLENDER_EEVEE_NEXT"
    except Exception:
        try:
            s.render.engine = "BLENDER_EEVEE"
        except Exception:
            s.render.engine = "CYCLES"
            s.cycles.samples = 40
    s.render.resolution_x = 768
    s.render.resolution_y = 1024
    s.render.image_settings.file_format = "PNG"
    s.render.film_transparent = True
    if not s.world:
        s.world = bpy.data.worlds.new("W")
    try:
        s.world.use_nodes = True
        bg = s.world.node_tree.nodes.get("Background")
        if bg:
            bg.inputs[0].default_value = (0.06, 0.05, 0.08, 1)
            bg.inputs[1].default_value = 0.4
    except Exception:
        pass


def add_lights():
    for name, loc, e in [("K", (2.4, -2.3, 2.3), 130), ("F", (-2.3, 0.8, 1.7), 42), ("R", (0.3, 2.8, 2.4), 38)]:
        ld = bpy.data.lights.new(name, type="AREA")
        ld.energy = e
        ld.size = 2.2
        o = bpy.data.objects.new(name, ld)
        o.location = loc
        bpy.context.scene.collection.objects.link(o)


def add_cam():
    cd = bpy.data.cameras.new("Cam")
    cd.lens = 55
    cam = bpy.data.objects.new("Cam", cd)
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    return cam


def look_at(cam, target, dist, angle_deg=0.0, z_off=0.0):
    ang = math.radians(angle_deg)
    cam.location = target + Vector((math.sin(ang) * dist, -math.cos(ang) * dist, z_off))
    cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()


def skin_mat(obj, color=(0.78, 0.55, 0.62, 1.0), name=None):
    mat = bpy.data.materials.new(name or f"skin_{obj.name}")
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs["Base Color"].default_value = color
        if "Roughness" in bsdf.inputs:
            bsdf.inputs["Roughness"].default_value = 0.55
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    return mat


def find_head_body():
    head = body = None
    for o in bpy.data.objects:
        if o.type != "MESH":
            continue
        n = o.name.lower()
        if "body" in n:
            body = o
        elif n.startswith("head") or "furry" in n:
            if head is None or len(o.data.vertices) > len(head.data.vertices):
                head = o
    return head, body


def world_center(obj):
    coords = [obj.matrix_world @ v.co for v in obj.data.vertices]
    return sum(coords, Vector()) / len(coords)


def manual_face_uv_island(head: bpy.types.Object, front_dot=0.25):
    """
    Build UV:
    - Front-facing polys (normal · -Y high) get projected island mapped into 0..1 plate space
    - Back/side polys get UV parked in unused corner with solid skin (no face tex)
    Two materials: face + skin via material index
    """
    mesh = head.data
    mw = head.matrix_world
    rot = mw.to_3x3()

    # bmesh edit
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bm.faces.ensure_lookup_table()
    uv_layer = bm.loops.layers.uv.new("FaceIsland") if not bm.loops.layers.uv else bm.loops.layers.uv[0]
    # recreate clean
    while bm.loops.layers.uv:
        bm.loops.layers.uv.remove(bm.loops.layers.uv[0])
    uv_layer = bm.loops.layers.uv.new("FaceIsland")

    # ensure 2 materials slots later
    front_faces = []
    back_faces = []
    # front axis in object space: world -Y
    front_world = Vector((0, -1, 0))
    front_obj = (rot.inverted() @ front_world).normalized()

    for f in bm.faces:
        n = f.normal.normalized()
        # also bias by face center y (more front)
        cen = f.calc_center_median()
        wcen = mw @ cen
        dot = n.dot(front_obj)
        # accept faces pointing forward enough OR on front half of head
        ys = [(mw @ v.co).y for v in f.verts]
        on_front = (sum(ys) / len(ys)) < (world_center(head).y + 0.02)
        if dot >= front_dot and on_front:
            front_faces.append(f)
            f.material_index = 1  # face
        else:
            back_faces.append(f)
            f.material_index = 0  # skin

    print(f"front_faces={len(front_faces)} back_faces={len(back_faces)}")
    if len(front_faces) < 5:
        # relax threshold
        front_faces, back_faces = [], []
        for f in bm.faces:
            n = f.normal.normalized()
            cen = mw @ f.calc_center_median()
            if n.dot(front_obj) >= 0.05 and cen.y < world_center(head).y + 0.05:
                front_faces.append(f)
                f.material_index = 1
            else:
                back_faces.append(f)
                f.material_index = 0
        print(f"relaxed front_faces={len(front_faces)}")

    # Collect front verts world positions for bounding box in camera plane (X, Z)
    pts = []
    for f in front_faces:
        for v in f.verts:
            w = mw @ v.co
            pts.append(w)
    if not pts:
        bm.to_mesh(mesh)
        bm.free()
        raise RuntimeError("no front faces")

    xs = [p.x for p in pts]
    zs = [p.z for p in pts]
    xmin, xmax = min(xs), max(xs)
    zmin, zmax = min(zs), max(zs)
    # pad slightly
    pad_x = (xmax - xmin) * 0.08 + 1e-5
    pad_z = (zmax - zmin) * 0.08 + 1e-5
    xmin -= pad_x
    xmax += pad_x
    zmin -= pad_z
    zmax += pad_z

    def to_uv(w: Vector):
        # plate is portrait: u = x, v = z (bottom to top)
        u = (w.x - xmin) / (xmax - xmin)
        v = (w.z - zmin) / (zmax - zmin)
        # FaceLock plate has head centered; optional inset
        u = 0.05 + 0.9 * u
        v = 0.05 + 0.9 * v
        return u, v

    for f in front_faces:
        for loop in f.loops:
            w = mw @ loop.vert.co
            u, v = to_uv(w)
            loop[uv_layer].uv = (u, v)

    # back faces: tiny UV at 0,0 (will use skin mat only — still set)
    for f in back_faces:
        for loop in f.loops:
            loop[uv_layer].uv = (0.01, 0.01)

    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    return len(front_faces)


def assign_face_and_skin_mats(head, face_path: Path):
    skin = bpy.data.materials.new("head_skin")
    skin.use_nodes = True
    bsdf = skin.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs["Base Color"].default_value = (0.78, 0.55, 0.62, 1)
        if "Roughness" in bsdf.inputs:
            bsdf.inputs["Roughness"].default_value = 0.55

    face = bpy.data.materials.new("head_face_island")
    face.use_nodes = True
    nt = face.node_tree
    nodes, links = nt.nodes, nt.links
    nodes.clear()
    out = nodes.new("ShaderNodeOutputMaterial")
    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    tex = nodes.new("ShaderNodeTexImage")
    img = bpy.data.images.load(str(face_path), check_existing=True)
    tex.image = img
    tex.interpolation = "Closest"
    tex.extension = "CLIP"
    uv = nodes.new("ShaderNodeUVMap")
    uv.uv_map = head.data.uv_layers[0].name if head.data.uv_layers else "FaceIsland"
    links.new(uv.outputs["UV"], tex.inputs["Vector"])
    links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    if "Roughness" in bsdf.inputs:
        bsdf.inputs["Roughness"].default_value = 0.55
    links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    head.data.materials.clear()
    head.data.materials.append(skin)  # 0
    head.data.materials.append(face)  # 1


def make_face_card(head, face_path: Path):
    """Backup: textured plane in front of face, parented to head."""
    center = world_center(head)
    coords = [head.matrix_world @ v.co for v in head.data.vertices]
    xs = [c.x for c in coords]
    zs = [c.z for c in coords]
    ys = [c.y for c in coords]
    width = (max(xs) - min(xs)) * 1.15
    height = (max(zs) - min(zs)) * 1.2
    # plane facing camera (+ default plane is XY); rotate to face -Y
    bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 0, 0))
    card = bpy.context.object
    card.name = "FaceCard"
    card.scale = (width / 2 * 1.0, height / 2 * 1.0, 1)
    # place slightly in front of head
    card.location = Vector((center.x, min(ys) - 0.02, center.z + height * 0.02))
    card.rotation_euler = (math.radians(90), 0, 0)  # stand upright facing -Y? 
    # Default plane is XY; rotate X 90 -> XZ plane, normal +Y. Want normal -Y so rotate X -90 or 90 and flip
    card.rotation_euler = (math.radians(90), 0, math.radians(180))

    # material
    mat = bpy.data.materials.new("face_card_mat")
    mat.use_nodes = True
    mat.blend_method = "CLIP"
    try:
        mat.shadow_method = "CLIP"
    except Exception:
        pass
    nt = mat.node_tree
    nodes, links = nt.nodes, nt.links
    nodes.clear()
    out = nodes.new("ShaderNodeOutputMaterial")
    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = bpy.data.images.load(str(face_path), check_existing=True)
    tex.interpolation = "Closest"
    # transparent where? plate has dark bg — use less than key; for now opaque
    links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    if "Roughness" in bsdf.inputs:
        bsdf.inputs["Roughness"].default_value = 0.5
    # optional alpha from dark bg
    sep = nodes.new("ShaderNodeSeparateColor") if hasattr(bpy.types, "ShaderNodeSeparateColor") else None
    links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    card.data.materials.append(mat)

    # parent to head keep transform
    card.parent = head
    # hide original head face? scale head face mats to skin only for card mode — handled by caller
    return card


def render_set(cam, head, body, tag: str, out_dir: Path):
    out_dir.mkdir(exist_ok=True)
    hc = world_center(head)
    hcoords = [head.matrix_world @ v.co for v in head.data.vertices]
    hh = max(c.z for c in hcoords) - min(c.z for c in hcoords)

    shots = [
        ("portrait_front", "p", 0),
        ("portrait_34", "p", 32),
        ("portrait_profile", "p", 85),
        ("full_front", "f", 0),
        ("full_34", "f", 40),
    ]
    all_m = [o for o in bpy.data.objects if o.type == "MESH"]
    for name, mode, ang in shots:
        if mode == "p":
            look_at(cam, hc, dist=max(0.7, hh * 2.7), angle_deg=ang)
        else:
            coords = []
            for o in all_m:
                for v in o.data.vertices:
                    coords.append(o.matrix_world @ v.co)
            c = sum(coords, Vector()) / len(coords)
            H = max(p.z for p in coords) - min(p.z for p in coords)
            look_at(cam, c, dist=max(3.3, H * 1.9), angle_deg=ang)
        path = out_dir / f"{tag}_{name}.png"
        bpy.context.scene.render.filepath = str(path)
        bpy.ops.render.render(write_still=True)
        print("R", path.name)


def main():
    bpy.ops.wm.open_mainfile(filepath=str(BLEND))
    setup_render()
    add_lights()
    cam = add_cam()

    head, body = find_head_body()
    print("HEAD", head.name if head else None, "BODY", body.name if body else None)
    if not head:
        raise SystemExit("no head")

    if body:
        skin_mat(body)
    for o in list(bpy.data.objects):
        if o.type == "MESH" and o not in (head, body) and "FaceCard" not in o.name:
            skin_mat(o)

    # --- PRIMARY: UV island ---
    nfront = manual_face_uv_island(head, front_dot=0.2)
    assign_face_and_skin_mats(head, FACE)
    bpy.ops.wm.save_as_mainfile(filepath=str(OUT / "femhuman_pass2_uv_island.blend"))
    render_set(cam, head, body, "uv", TURNS)

    # --- BACKUP: face card (reload clean body state from cinch, then card) ---
    bpy.ops.wm.open_mainfile(filepath=str(BLEND))
    setup_render()
    # clear leftover lights names by adding fresh
    add_lights()
    cam = add_cam()
    head, body = find_head_body()
    if body:
        skin_mat(body)
    for o in bpy.data.objects:
        if o.type == "MESH" and o not in (head, body):
            skin_mat(o)
    # head solid skin under card
    skin_mat(head)
    card = make_face_card(head, FACE)
    # Slightly shrink head so card reads? optional
    bpy.ops.wm.save_as_mainfile(filepath=str(OUT / "femhuman_pass2_face_card.blend"))
    render_set(cam, head, body, "card", CARD)

    print("PASS2_OK", "front_faces", nfront)


main()
