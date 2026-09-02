"""
Face-map pass 1: banked female plate → femhumandx head (UV project / decal).
Render front + turnaround.
"""
from __future__ import annotations

import math
from pathlib import Path

import bpy
from mathutils import Vector, Matrix

ROOT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/walkers/bodies_dx")
OUT = ROOT / "face_map_pass1"
TURNS = OUT / "turns"
OUT.mkdir(exist_ok=True)
TURNS.mkdir(exist_ok=True)

FACE = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/walkers/out/paperdoll_v0/ship_v0/heads/head_a_human_androgynous_v0.jpg")
if not FACE.exists():
    FACE = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/walkers/facelock_plate_v1_ref.jpg")

BLEND = ROOT / "exports" / "femhumandx_waist_cinch.blend"
if not BLEND.exists():
    BLEND = ROOT / "src" / "femhumandx.blend"


def setup_cycles_or_eevee():
    s = bpy.context.scene
    try:
        s.render.engine = "BLENDER_EEVEE_NEXT"
    except Exception:
        try:
            s.render.engine = "BLENDER_EEVEE"
        except Exception:
            s.render.engine = "CYCLES"
            s.cycles.samples = 64
    s.render.resolution_x = 768
    s.render.resolution_y = 1024
    s.render.image_settings.file_format = "PNG"
    s.render.film_transparent = True
    if s.world is None:
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
    for name, loc, energy in [
        ("Key", (2.2, -2.5, 2.2), 120),
        ("Fill", (-2.5, 0.5, 1.8), 40),
        ("Rim", (0.5, 2.5, 2.5), 35),
    ]:
        ld = bpy.data.lights.new(name, type="AREA")
        ld.energy = energy
        ld.size = 2.0
        o = bpy.data.objects.new(name, ld)
        o.location = loc
        bpy.context.scene.collection.objects.link(o)


def add_camera():
    cd = bpy.data.cameras.new("Cam")
    cd.lens = 55
    cam = bpy.data.objects.new("Cam", cd)
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    return cam


def frame_full(cam, objs, angle_deg=0.0):
    coords = []
    for o in objs:
        if o.type != "MESH":
            continue
        for v in o.data.vertices:
            coords.append(o.matrix_world @ v.co)
    if not coords:
        return
    center = sum(coords, Vector()) / len(coords)
    zs = [c.z for c in coords]
    h = max(zs) - min(zs)
    dist = max(3.2, h * 1.9)
    ang = math.radians(angle_deg)
    cam.location = Vector((math.sin(ang) * dist, -math.cos(ang) * dist, center.z * 0.95))
    direction = center - cam.location
    cam.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()


def frame_portrait(cam, head_obj, angle_deg=0.0):
    coords = [head_obj.matrix_world @ v.co for v in head_obj.data.vertices]
    center = sum(coords, Vector()) / len(coords)
    # slightly toward face front (-Y)
    zs = [c.z for c in coords]
    h = max(zs) - min(zs)
    dist = max(0.55, h * 2.4)
    ang = math.radians(angle_deg)
    # orbit around head
    cam.location = center + Vector((math.sin(ang) * dist, -math.cos(ang) * dist, h * 0.05))
    direction = center - cam.location
    cam.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()


def ensure_uv(obj):
    mesh = obj.data
    if not mesh.uv_layers:
        # smart project via ops needs context — create simple UV
        mesh.uv_layers.new(name="UVMap")
    # try smart unwrap with override
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    try:
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.uv.smart_project(angle_limit=math.radians(66), island_margin=0.02)
        bpy.ops.object.mode_set(mode="OBJECT")
    except Exception as e:
        print("UV unwrap fallback", e)
        try:
            bpy.ops.object.mode_set(mode="OBJECT")
        except Exception:
            pass


def face_material(obj, image_path: Path, name="face_map"):
    """Principled with image texture; use UV. Also add strong emission mix for readability."""
    img = bpy.data.images.load(str(image_path), check_existing=True)
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    nodes = nt.nodes
    links = nt.links
    nodes.clear()
    out = nodes.new("ShaderNodeOutputMaterial")
    out.location = (400, 0)
    bsdf = nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.location = (100, 0)
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = img
    tex.location = (-300, 0)
    tex.interpolation = "Closest"  # pixel look
    uv = nodes.new("ShaderNodeUVMap")
    uv.location = (-500, 0)
    # mapping to tweak
    mapping = nodes.new("ShaderNodeMapping")
    mapping.location = (-400, 0)
    texcoord = nodes.new("ShaderNodeTexCoord")
    texcoord.location = (-600, 0)
    links.new(texcoord.outputs["UV"], mapping.inputs["Vector"])
    links.new(mapping.outputs["Vector"], tex.inputs["Vector"])
    links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    if "Roughness" in bsdf.inputs:
        bsdf.inputs["Roughness"].default_value = 0.65
    links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    # assign
    mesh = obj.data
    mesh.materials.clear()
    mesh.materials.append(mat)
    return mat, mapping, tex


def body_mat(obj):
    mat = bpy.data.materials.get("body_skin") or bpy.data.materials.new("body_skin")
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs["Base Color"].default_value = (0.78, 0.55, 0.62, 1)
        if "Roughness" in bsdf.inputs:
            bsdf.inputs["Roughness"].default_value = 0.55
    if obj.data.materials:
        obj.data.materials[0] = mat
    else:
        obj.data.materials.append(mat)


def project_from_view_camera(obj, cam):
    """UV project from camera (front)."""
    # Use UV project modifier
    mod = obj.modifiers.new(name="FaceProject", type="UV_PROJECT")
    mod.projector_count = 1
    mod.projectors[0].object = cam
    if obj.data.uv_layers:
        mod.uv_layer = obj.data.uv_layers[0].name
    # apply
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    try:
        bpy.ops.object.modifier_apply(modifier=mod.name)
        print("UV_PROJECT applied")
    except Exception as e:
        print("UV_PROJECT apply failed", e)


def main():
    bpy.ops.wm.open_mainfile(filepath=str(BLEND))
    setup_cycles_or_eevee()

    meshes = [o for o in bpy.data.objects if o.type == "MESH"]
    head = None
    body = None
    extras = []
    for o in meshes:
        n = o.name.lower()
        if n.startswith("head") and "body" not in n:
            # main head is largest head mesh
            if head is None or len(o.data.vertices) > len(head.data.vertices):
                if head is not None:
                    extras.append(head)
                head = o
            else:
                extras.append(o)
        elif "body" in n:
            body = o
        else:
            extras.append(o)

    print("HEAD", head.name if head else None, "BODY", body.name if body else None)
    if head is None:
        raise SystemExit("no head")

    # hide tiny head extras (eyes/teeth bits) or leave — keep for shape
    for o in extras:
        if o != head and len(o.data.vertices) < 200:
            # keep mouth bits visible with body skin
            body_mat(o)

    if body:
        body_mat(body)

    # Front camera for projection first
    cam = add_camera()
    add_lights()
    frame_portrait(cam, head, angle_deg=0)

    # UV: smart project then camera project from front
    ensure_uv(head)
    # Align a temporary ortho-ish front cam for projection
    frame_portrait(cam, head, 0)
    project_from_view_camera(head, cam)

    mat, mapping, tex = face_material(head, FACE, "face_female_banked")
    # Tune mapping — scale up face on UV a bit
    mapping.inputs["Scale"].default_value = (1.0, 1.0, 1.0)

    # Save blend
    out_blend = OUT / "femhuman_face_map_pass1.blend"
    bpy.ops.wm.save_as_mainfile(filepath=str(out_blend))
    print("SAVED", out_blend)

    # Renders: portrait front + 3/4 + full body front + turnaround
    shots = [
        ("portrait_front", "portrait", 0),
        ("portrait_34", "portrait", 35),
        ("portrait_profile", "portrait", 80),
        ("full_front", "full", 0),
        ("full_34", "full", 40),
        ("full_back", "full", 180),
    ]
    all_meshes = [o for o in bpy.data.objects if o.type == "MESH"]
    for name, mode, ang in shots:
        if mode == "portrait":
            frame_portrait(cam, head, ang)
            bpy.context.scene.render.resolution_x = 768
            bpy.context.scene.render.resolution_y = 1024
        else:
            frame_full(cam, all_meshes, ang)
            bpy.context.scene.render.resolution_x = 768
            bpy.context.scene.render.resolution_y = 1024
        path = TURNS / f"{name}.png"
        bpy.context.scene.render.filepath = str(path)
        bpy.ops.render.render(write_still=True)
        print("RENDER", path)

    # turnaround strip angles
    for ang in [0, 45, 90, 135, 180, 225, 270, 315]:
        frame_full(cam, all_meshes, ang)
        path = TURNS / f"turn_{ang:03d}.png"
        bpy.context.scene.render.filepath = str(path)
        bpy.ops.render.render(write_still=True)
        print("TURN", ang)

    # stitch turn strip with PIL outside — write list
    print("PASS1_OK")


main()
