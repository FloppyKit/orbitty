"""
Vivi idle v3: DIRECT mesh sectional deformation each frame (guaranteed body motion).
Not paper-doll whole-object bob. Arms/torso/head/legs move as mesh regions.
"""
from __future__ import annotations

import json
import math
import shutil
from pathlib import Path

import bpy
from mathutils import Vector, Matrix, Euler

VIVI_DIR = Path.home() / "Downloads/PlayStation 2 - Kingdom Hearts II - Characters (Final Fantasy) - Vivi/Vivi"
OBJ = VIVI_DIR / "Vivi.obj"
OUT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion/vivi_rig_idle")
PACK = Path.home() / "Library/Application Support/space.orbitty.mac/packs/companion-vivi-rig-idle"
W, H, NFRAMES = 256, 384, 12


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def setup():
    s = bpy.context.scene
    try:
        s.render.engine = "BLENDER_EEVEE_NEXT"
    except Exception:
        try:
            s.render.engine = "BLENDER_EEVEE"
        except Exception:
            s.render.engine = "CYCLES"
            s.cycles.samples = 24
    s.render.resolution_x = W
    s.render.resolution_y = H
    s.render.film_transparent = True
    s.render.image_settings.file_format = "PNG"
    s.render.image_settings.color_mode = "RGBA"
    if not s.world:
        s.world = bpy.data.worlds.new("W")
    try:
        s.world.use_nodes = True
        bg = s.world.node_tree.nodes.get("Background")
        if bg:
            bg.inputs[0].default_value = (0.7, 0.75, 0.85, 1)
            bg.inputs[1].default_value = 1.0
    except Exception:
        pass
    for name, loc, e, size in [
        ("Key", (3.0, -4.0, 5.0), 700, 5),
        ("Fill", (-4.0, -1.5, 3.0), 250, 6),
        ("Rim", (0.2, 5.0, 3.5), 180, 4),
        ("Front", (0.0, -5.5, 2.8), 280, 5),
    ]:
        ld = bpy.data.lights.new(name, type="AREA")
        ld.energy = e
        ld.size = size
        o = bpy.data.objects.new(name, ld)
        o.location = loc
        s.collection.objects.link(o)


def import_body():
    import os
    os.chdir(VIVI_DIR)
    before = set(bpy.data.objects)
    try:
        bpy.ops.wm.obj_import(filepath=str(OBJ))
    except Exception:
        bpy.ops.import_scene.obj(filepath=str(OBJ))
    meshes = [o for o in bpy.data.objects if o not in before and o.type == "MESH"]
    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()
    body = bpy.context.view_layer.objects.active
    body.name = "ViviBody"
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    return body


def rest_coords(body):
    """Store rest local coords."""
    return [v.co.copy() for v in body.data.vertices]


def bounds_from_coords(coords):
    xs = [p.x for p in coords]
    ys = [p.y for p in coords]
    zs = [p.z for p in coords]
    mn = Vector((min(xs), min(ys), min(zs)))
    mx = Vector((max(xs), max(ys), max(zs)))
    return mn, mx, (mn + mx) * 0.5, mx - mn


def deform(body, rest, t):
    """
    t in radians 0..2pi.
    Sectional twist/bend: head, chest, arms (by x), hips, legs — around local pivots.
    """
    mn, mx, center, size = bounds_from_coords(rest)
    z0, z1 = mn.z, mx.z
    h = max(z1 - z0, 1e-5)
    mid_x = center.x
    mid_y = center.y

    breath = math.sin(t) * 0.09
    sway = math.sin(t * 0.5) * 0.07
    arm_phase = math.sin(t) * 0.18
    head_turn = math.sin(t * 0.5) * 0.11
    bob = math.sin(t) * (h * 0.012)

    mesh = body.data
    for i, v in enumerate(mesh.vertices):
        p = rest[i].copy()
        tt = (p.z - z0) / h  # 0..1
        lr = p.x - mid_x

        # pivots
        hip_z = z0 + h * 0.45
        chest_z = z0 + h * 0.62
        neck_z = z0 + h * 0.78

        # whole small bob
        p.z += bob

        # legs weight shift (below hips)
        if tt < 0.45:
            side = 1.0 if lr >= 0 else -1.0
            knee = abs(math.sin(t * 0.5 + (0 if lr >= 0 else math.pi))) * 0.12
            # bend forward around knee height
            knee_z = z0 + h * 0.22
            if p.z < hip_z:
                rel = Vector((p.x - (mid_x + side * size.x * 0.08), p.y - mid_y, p.z - knee_z))
                rot = Matrix.Rotation(knee * (1.0 if p.z < knee_z else 0.35), 4, "X")
                rel = rot @ rel
                p = Vector((mid_x + side * size.x * 0.08, mid_y, knee_z)) + rel
                p.x += sway * side * 0.15 * h * (1 - tt)

        # hips / torso twist
        if 0.40 < tt < 0.75:
            rel = Vector((p.x - mid_x, p.y - mid_y, p.z - hip_z))
            rot = Matrix.Rotation(sway * 0.8, 4, "Z") @ Matrix.Rotation(breath * 0.5, 4, "X")
            rel = rot @ rel
            p = Vector((mid_x, mid_y, hip_z)) + rel
            # chest expand-ish
            p.x += (p.x - mid_x) * breath * 0.15

        # chest/shoulders
        if tt >= 0.58:
            rel = Vector((p.x - mid_x, p.y - mid_y, p.z - chest_z))
            rot = Matrix.Rotation(breath, 4, "X")
            rel = rot @ rel
            p = Vector((mid_x, mid_y, chest_z)) + rel

        # arms: outer vertices swing
        if abs(lr) > size.x * 0.20 and 0.35 < tt < 0.78:
            side = 1.0 if lr > 0 else -1.0
            shoulder = Vector((mid_x + side * size.x * 0.22, mid_y, z0 + h * 0.68))
            rel = p - shoulder
            swing = arm_phase * side
            # swing primarily in YZ around shoulder (X axis) and slight Z
            rot = (
                Matrix.Rotation(0.2 + swing, 4, "X")
                @ Matrix.Rotation(side * 0.25, 4, "Z")
            )
            p = shoulder + rot @ rel

        # head
        if tt > 0.80:
            rel = Vector((p.x - mid_x, p.y - mid_y, p.z - neck_z))
            rot = (
                Matrix.Rotation(-breath * 0.3, 4, "X")
                @ Matrix.Rotation(head_turn, 4, "Z")
                @ Matrix.Rotation(math.cos(t * 0.5) * 0.06, 4, "Y")
            )
            p = Vector((mid_x, mid_y, neck_z)) + rot @ rel

        v.co = p

    mesh.update()


def frame_cam(cam, body):
    coords = [body.matrix_world @ v.co for v in body.data.vertices]
    xs, ys, zs = [p.x for p in coords], [p.y for p in coords], [p.z for p in coords]
    mn = Vector((min(xs), min(ys), min(zs)))
    mx = Vector((max(xs), max(ys), max(zs)))
    center = (mn + mx) * 0.5
    size = mx - mn
    h = max(size.z, 0.01)
    dist = max(size.length * 1.1, h * 1.95)
    cam.location = Vector((center.x, center.y - dist, center.z))
    cam.rotation_euler = (center - cam.location).to_track_quat("-Z", "Y").to_euler()


def main():
    reset()
    setup()
    body = import_body()
    rest = rest_coords(body)
    print("verts", len(rest), "size", bounds_from_coords(rest)[3])

    cd = bpy.data.cameras.new("C")
    cd.lens = 45
    cam = bpy.data.objects.new("C", cd)
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    # camera from rest pose
    deform(body, rest, 0)  # ensure rest
    # reset to rest
    for i, v in enumerate(body.data.vertices):
        v.co = rest[i]
    body.data.update()
    frame_cam(cam, body)

    frames_dir = OUT / "frames"
    if frames_dir.exists():
        shutil.rmtree(frames_dir)
    frames_dir.mkdir(parents=True)
    if PACK.exists():
        shutil.rmtree(PACK)
    PACK.mkdir(parents=True)

    bpy.ops.wm.save_as_mainfile(filepath=str(OUT / "vivi_section_idle.blend"))

    for i in range(NFRAMES):
        t = i / NFRAMES * 2 * math.pi
        deform(body, rest, t)
        path = frames_dir / f"idle_{i}.png"
        bpy.context.scene.render.filepath = str(path)
        bpy.ops.render.render(write_still=True)
        shutil.copy2(path, PACK / f"idle_{i}.png")
        shutil.copy2(path, PACK / f"walk_{i}.png")
        print("frame", i)

    shutil.copy2(frames_dir / "idle_0.png", PACK / "idle.png")
    man = {
        "name": "Vivi section idle (temp)",
        "license": "dev-temp-not-for-ship",
        "notes": "Sectional mesh deform idle (head/chest/arms/legs) — not whole-sprite bob",
        "frames": [f"idle_{i}.png" for i in range(NFRAMES)],
        "idle_frames": [f"idle_{i}.png" for i in range(NFRAMES)],
    }
    (OUT / "manifest.json").write_text(json.dumps(man, indent=2))
    (PACK / "manifest.json").write_text(json.dumps(man, indent=2))
    (OUT / "NOTES.md").write_text(
        """# Vivi companion idle experiment

## Method v3
Sectional mesh deformation (height/X bands) each frame — arms, torso breath, head turn, weight shift.
Armature auto-weight failed on this rip (bone heat + no deformation).

## Outputs
- frames/idle_0..11.png (256×384)
- packs/companion-vivi-rig-idle (local app support)
- NOT wired into companion UI (inspect first)

## Limits
- Still a PS2 rip; MTL path broken (Thing.mtl) so textures may be incomplete
- Not game animation clips — procedural idle on the mesh
- For ship: need own character + real rig or exported clips
"""
    )
    print("OK")


main()
