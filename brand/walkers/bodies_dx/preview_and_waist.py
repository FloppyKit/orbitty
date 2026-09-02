"""Preview all 4 DX bodies + cinch female waist. Blender 5 safe."""
from __future__ import annotations

import math
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/walkers/bodies_dx")
SRC = ROOT / "src"
PREV = ROOT / "previews"
EXP = ROOT / "exports"
PREV.mkdir(exist_ok=True)
EXP.mkdir(exist_ok=True)

FILES = [
    "femhumandx.blend",
    "femcaninedx.blend",
    "maschumandx.blend",
    "masccaninedx.blend",
]


def ensure_view_layer():
    win = bpy.context.window
    if win is None:
        return
    # touch scene
    bpy.context.view_layer.update()


def setup_render(path: Path, res=512):
    scene = bpy.context.scene
    try:
        scene.render.engine = "BLENDER_EEVEE_NEXT"
    except Exception:
        try:
            scene.render.engine = "BLENDER_EEVEE"
        except Exception:
            scene.render.engine = "CYCLES"
            scene.cycles.samples = 32
    scene.render.resolution_x = res
    scene.render.resolution_y = int(res * 1.25)
    scene.render.filepath = str(path)
    scene.render.image_settings.file_format = "PNG"
    scene.render.film_transparent = True
    if scene.world is None:
        scene.world = bpy.data.worlds.new("World")
    try:
        scene.world.use_nodes = True
        bg = scene.world.node_tree.nodes.get("Background")
        if bg:
            bg.inputs[0].default_value = (0.07, 0.06, 0.09, 1)
            bg.inputs[1].default_value = 0.35
    except Exception:
        pass


def add_light_cam(height=1.0):
    """Create lights/camera via data API (no ops context issues)."""
    # key
    light_data = bpy.data.lights.new(name="Key", type="AREA")
    light_data.energy = 100
    light_data.size = 2.5
    light_obj = bpy.data.objects.new("Key", light_data)
    light_obj.location = (2.5, -2.5, height + 0.8)
    bpy.context.scene.collection.objects.link(light_obj)
    # fill
    fill_data = bpy.data.lights.new(name="Fill", type="AREA")
    fill_data.energy = 30
    fill_data.size = 3
    fill_obj = bpy.data.objects.new("Fill", fill_data)
    fill_obj.location = (-2.0, 1.2, height + 0.3)
    bpy.context.scene.collection.objects.link(fill_obj)
    # cam
    cam_data = bpy.data.cameras.new("Cam")
    cam_data.lens = 50
    cam = bpy.data.objects.new("Cam", cam_data)
    cam.location = (0.0, -3.4, height * 0.55)
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    return cam


def frame_camera(cam, objs):
    coords = []
    for o in objs:
        if o.type != "MESH" or not o.data or not len(o.data.vertices):
            continue
        for v in o.data.vertices:
            coords.append(o.matrix_world @ v.co)
    if not coords:
        return
    center = sum(coords, Vector()) / len(coords)
    zs = [c.z for c in coords]
    h = max(zs) - min(zs) + 0.01
    cam.location = Vector((0.0, -max(3.0, h * 1.85), float(center.z)))
    direction = center - cam.location
    cam.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()


def apply_pink_mat(obj):
    mat = bpy.data.materials.get("bitty_skin") or bpy.data.materials.new("bitty_skin")
    try:
        mat.use_nodes = True
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        if bsdf:
            bsdf.inputs["Base Color"].default_value = (0.78, 0.55, 0.62, 1.0)
            if "Roughness" in bsdf.inputs:
                bsdf.inputs["Roughness"].default_value = 0.55
    except Exception:
        pass
    if obj.data.materials:
        obj.data.materials[0] = mat
    else:
        obj.data.materials.append(mat)


def cinch_waist(body, factor=0.86, band=0.14):
    mesh = body.data
    zs = [v.co.z for v in mesh.vertices]
    zmin, zmax = min(zs), max(zs)
    mid = zmin + (zmax - zmin) * 0.57
    half = max(band * (zmax - zmin), 0.05)
    for v in mesh.vertices:
        t = abs(v.co.z - mid) / half
        if t >= 1.0:
            continue
        w = (1.0 - t) ** 2
        s = 1.0 + (factor - 1.0) * w
        v.co.x *= s
        v.co.y *= s
    mesh.update()


def process(path: Path, cinch: bool = False, out_name: str | None = None):
    bpy.ops.wm.open_mainfile(filepath=str(path))
    ensure_view_layer()
    meshes = [o for o in bpy.data.objects if o.type == "MESH"]
    body = None
    for o in meshes:
        apply_pink_mat(o)
        if "body" in o.name.lower():
            body = o
    if cinch and body is not None:
        cinch_waist(body, factor=0.86, band=0.14)
        print("CINCHED", path.name, body.name)

    stem = out_name or path.stem
    setup_render(PREV / f"{stem}.png")
    # remove existing lights/cams we might double if any leftover names
    cam = add_light_cam(height=1.0)
    frame_camera(cam, meshes)
    bpy.ops.render.render(write_still=True)
    print("PREVIEW", stem)

    if cinch:
        out_blend = EXP / f"{path.stem}_waist_cinch.blend"
        bpy.ops.wm.save_as_mainfile(filepath=str(out_blend))
        print("SAVED", out_blend)


for fn in FILES:
    process(SRC / fn, cinch=False)

process(SRC / "femhumandx.blend", cinch=True, out_name="femhumandx_waist_cinch")
process(SRC / "femcaninedx.blend", cinch=True, out_name="femcaninedx_waist_cinch")
print("ALL_OK")
