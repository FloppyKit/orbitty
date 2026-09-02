"""
Orbitty planet walker sheet v0 — Blender 4.x/5.x headless.

AxisLock: DQ8 × Skies of Arcadia, slight chibi, cel-ish, warm travelogue.
Never invent past brand/walkers/AXISLOCK.md.

Usage:
  blender --background --python sheet_walker_v0.py -- \
      --species traveler_m --out /path/to/out

  blender --background --python sheet_walker_v0.py -- \
      --species slime_guide --out /path/to/out

Emits: {out}/{species}_walk_sheet.png and per-frame PNGs.
Exciting later = better meshes/materials; this proves multi-species factory.
"""
from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import bpy
from mathutils import Euler, Vector


def parse_args():
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1 :]
    else:
        argv = []
    p = argparse.ArgumentParser()
    p.add_argument(
        "--species",
        default="traveler_m",
        choices=["traveler_m", "traveler_f", "slime_guide", "birdfolk_scout"],
    )
    p.add_argument("--out", required=True)
    p.add_argument("--frames", type=int, default=4)
    p.add_argument("--res", type=int, default=256)
    p.add_argument("--glitch", type=float, default=0.35,
                   help="polygon facet jitter 0..1 (Matt motion lock)")
    return p.parse_args(argv)


def clear_scene():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for block in bpy.data.meshes:
        if block.users == 0:
            bpy.data.meshes.remove(block)
    for block in bpy.data.materials:
        if block.users == 0:
            bpy.data.materials.remove(block)


def mat(name: str, color: tuple[float, float, float, float], rough=0.55):
    m = bpy.data.materials.new(name=name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs["Base Color"].default_value = color
        if "Roughness" in bsdf.inputs:
            bsdf.inputs["Roughness"].default_value = rough
        # Blender 4+ specular / coat varies — keep simple
    return m


def add_mesh(name: str, verts, faces, location, material):
    mesh = bpy.data.meshes.new(name + "_mesh")
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(obj)
    obj.location = location
    if material:
        obj.data.materials.append(material)
    return obj


def icosphere(name, radius, location, material, subdivisions=2):
    bpy.ops.mesh.primitive_ico_sphere_add(
        radius=radius, subdivisions=subdivisions, location=location
    )
    obj = bpy.context.active_object
    obj.name = name
    if material:
        if obj.data.materials:
            obj.data.materials[0] = material
        else:
            obj.data.materials.append(material)
    return obj


def cylinder(name, radius, depth, location, material):
    bpy.ops.mesh.primitive_cylinder_add(
        radius=radius, depth=depth, location=location
    )
    obj = bpy.context.active_object
    obj.name = name
    if material:
        obj.data.materials.append(material)
    return obj


def cube(name, scale, location, material):
    bpy.ops.mesh.primitive_cube_add(size=1, location=location)
    obj = bpy.context.active_object
    obj.name = name
    obj.scale = scale
    bpy.ops.object.transform_apply(scale=True)
    if material:
        obj.data.materials.append(material)
    return obj


def build_traveler(species: str):
    """Slight-chibi biped blocks — head large (AxisLock 1.5–2× feel)."""
    if species == "traveler_f":
        skin = mat("skin", (0.96, 0.82, 0.72, 1))
        hair_c = mat("hair", (0.75, 0.22, 0.18, 1))
        vest_c = mat("vest", (0.45, 0.28, 0.72, 1))
        skirt_c = mat("skirt", (0.65, 0.12, 0.18, 1))
        boot_c = mat("boot", (0.35, 0.25, 0.18, 1))
    else:
        skin = mat("skin", (0.92, 0.75, 0.58, 1))
        hair_c = mat("hair", (0.35, 0.22, 0.12, 1))
        vest_c = mat("vest", (0.85, 0.62, 0.18, 1))  # mustard
        skirt_c = mat("pants", (0.25, 0.40, 0.70, 1))
        boot_c = mat("boot", (0.30, 0.22, 0.15, 1))
        band = mat("bandana", (0.90, 0.40, 0.12, 1))

    parts = []
    # proportions: head big
    head = icosphere("head", 0.28, (0, 0, 1.55), skin, 3)
    parts.append(head)
    hair = icosphere("hair", 0.30, (0, -0.02, 1.60), hair_c, 2)
    hair.scale = (1.05, 1.1, 0.85)
    parts.append(hair)
    if species == "traveler_m":
        parts.append(cube("bandana", (0.32, 0.12, 0.08), (0, -0.22, 1.68), band))
    else:
        # twin-tail blobs
        parts.append(icosphere("tail_L", 0.08, (-0.22, -0.05, 1.45), hair_c, 1))
        parts.append(icosphere("tail_R", 0.08, (0.22, -0.05, 1.45), hair_c, 1))

    torso = cube("torso", (0.35, 0.22, 0.40), (0, 0, 1.05), vest_c)
    parts.append(torso)
    hips = cube("hips", (0.32, 0.20, 0.22), (0, 0, 0.72), skirt_c)
    parts.append(hips)

    # arms
    for side, x in (("L", -0.42), ("R", 0.42)):
        parts.append(cylinder(f"arm_{side}", 0.07, 0.45, (x, 0, 1.05), skin))
    # legs (animated later via location)
    leg_L = cylinder("leg_L", 0.09, 0.50, (-0.12, 0, 0.35), skirt_c)
    leg_R = cylinder("leg_R", 0.09, 0.50, (0.12, 0, 0.35), skirt_c)
    parts.extend([leg_L, leg_R])
    parts.append(cube("boot_L", (0.12, 0.18, 0.10), (-0.12, 0.02, 0.08), boot_c))
    parts.append(cube("boot_R", (0.12, 0.18, 0.10), (0.12, 0.02, 0.08), boot_c))

    # bag
    bag = cube("bag", (0.18, 0.12, 0.22), (0.28, -0.15, 0.95), mat("bag", (0.55, 0.35, 0.2, 1)))
    parts.append(bag)

    root = bpy.data.objects.new("Root", None)
    bpy.context.collection.objects.link(root)
    for o in parts:
        o.parent = root
    return root, leg_L, leg_R, head


def build_slime():
    body_m = mat("slime", (0.95, 0.55, 0.45, 1), rough=0.25)
    dark_m = mat("slime_dark", (0.85, 0.35, 0.30, 1), rough=0.3)
    flag_m = mat("flag", (0.2, 0.55, 0.85, 1))
    root = bpy.data.objects.new("Root", None)
    bpy.context.collection.objects.link(root)
    body = icosphere("body", 0.55, (0, 0, 0.50), body_m, 3)
    body.scale = (1.1, 0.95, 0.85)
    bump = icosphere("bump", 0.22, (0.15, -0.25, 0.75), dark_m, 2)
    # eyes — simple dark spheres (not wet VTuber)
    eye_m = mat("eye", (0.08, 0.08, 0.1, 1), rough=0.9)
    eL = icosphere("eye_L", 0.06, (-0.15, -0.42, 0.62), eye_m, 1)
    eR = icosphere("eye_R", 0.06, (0.15, -0.42, 0.62), eye_m, 1)
    pole = cylinder("pole", 0.02, 0.35, (0.05, 0, 1.05), mat("pole", (0.4, 0.3, 0.2, 1)))
    flag = cube("flag", (0.18, 0.02, 0.12), (0.18, 0, 1.15), flag_m)
    for o in (body, bump, eL, eR, pole, flag):
        o.parent = root
    return root, body, None, None


def build_birdfolk():
    skin = mat("feather", (0.85, 0.75, 0.55, 1))
    beak_m = mat("beak", (0.9, 0.55, 0.15, 1))
    scarf_m = mat("scarf", (0.2, 0.55, 0.75, 1))
    boot_c = mat("boot", (0.3, 0.2, 0.15, 1))
    root = bpy.data.objects.new("Root", None)
    bpy.context.collection.objects.link(root)
    parts = []
    head = icosphere("head", 0.26, (0, 0, 1.50), skin, 3)
    beak = cube("beak", (0.08, 0.18, 0.08), (0, -0.28, 1.45), beak_m)
    torso = cube("torso", (0.32, 0.22, 0.38), (0, 0, 1.05), skin)
    scarf = cube("scarf", (0.4, 0.08, 0.12), (0, -0.15, 1.28), scarf_m)
    scarf2 = cube("scarf_tail", (0.1, 0.35, 0.08), (0.15, -0.35, 1.1), scarf_m)
    leg_L = cylinder("leg_L", 0.07, 0.45, (-0.1, 0, 0.4), skin)
    leg_R = cylinder("leg_R", 0.07, 0.45, (0.1, 0, 0.4), skin)
    for o in (head, beak, torso, scarf, scarf2, leg_L, leg_R):
        o.parent = root
        parts.append(o)
    return root, leg_L, leg_R, head


def setup_world_camera(res: int):
    # Soft PS2-ish light
    sun = bpy.data.objects.new("Sun", bpy.data.lights.new("SunData", type="SUN"))
    bpy.context.collection.objects.link(sun)
    sun.data.energy = 2.5
    sun.rotation_euler = Euler((math.radians(45), math.radians(15), math.radians(30)))

    fill = bpy.data.objects.new("Fill", bpy.data.lights.new("FillData", type="AREA"))
    bpy.context.collection.objects.link(fill)
    fill.data.energy = 40
    fill.location = (-2, -2, 2)

    cam_data = bpy.data.cameras.new("Cam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = 2.4
    cam = bpy.data.objects.new("Camera", cam_data)
    bpy.context.collection.objects.link(cam)
    cam.location = (0, -4.0, 1.0)
    cam.rotation_euler = Euler((math.radians(90), 0, 0))
    bpy.context.scene.camera = cam

    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE_NEXT" if "BLENDER_EEVEE_NEXT" in {
        e.identifier for e in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items
    } else "BLENDER_EEVEE"
    # fallback
    try:
        scene.render.engine = "BLENDER_EEVEE_NEXT"
    except Exception:
        try:
            scene.render.engine = "EEVEE"
        except Exception:
            scene.render.engine = "CYCLES"

    scene.render.resolution_x = res
    scene.render.resolution_y = res
    scene.render.film_transparent = True
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"

    # gentle backdrop world
    world = bpy.data.worlds.new("World")
    scene.world = world
    world.use_nodes = True
    bg = world.node_tree.nodes.get("Background")
    if bg:
        bg.inputs[0].default_value = (0.91, 0.94, 0.90, 1)
        bg.inputs[1].default_value = 0.4



def apply_polygon_glitch(root, frame_i: int, n: int, amount: float):
    """Slight facet glitch with movement — interior meshes jitter, subtle."""
    if amount <= 0.001:
        return
    import random
    rng = random.Random(frame_i * 997 + 13)
    phase = frame_i / max(n, 1)
    # walk-coupled pulse
    pulse = 0.5 + 0.5 * abs(__import__("math").sin(phase * __import__("math").tau))
    amp = 0.012 * amount * (0.55 + 0.45 * pulse)

    def jitter_mesh(obj):
        if not obj.data or not hasattr(obj.data, "vertices"):
            return
        mesh = obj.data
        # use bmesh-free: move verts in object space slightly
        for vi, v in enumerate(mesh.vertices):
            # less on extreme silhouette-ish by damping with |x|
            damp = 1.0 / (1.0 + abs(v.co.x) * 1.2)
            ox = (rng.random() - 0.5) * 2 * amp * damp
            oy = (rng.random() - 0.5) * 2 * amp * damp * 0.6
            oz = (rng.random() - 0.5) * 2 * amp * damp * 0.8
            # stable base: store? for v0 we jitter from current — rebuild each frame from rest
            pass

    # Better approach: scale children slightly non-uniform + tiny rotation jitter
    for i, c in enumerate(list(root.children)):
        rng2 = random.Random(frame_i * 131 + i * 17)
        sx = 1.0 + (rng2.random() - 0.5) * 0.04 * amount * pulse
        sy = 1.0 + (rng2.random() - 0.5) * 0.04 * amount * pulse
        sz = 1.0 + (rng2.random() - 0.5) * 0.05 * amount * pulse
        # don't destroy intentional scales entirely — multiply gently if close to 1
        base = c.scale.copy()
        # only glitch if roughly unit-ish parts; for all, additive micro
        c.scale = (
            base.x * sx,
            base.y * sy,
            base.z * sz,
        )
        # micro rotation glitch on limbs/head more than root
        c.rotation_euler.x += (rng2.random() - 0.5) * 0.06 * amount
        c.rotation_euler.z += (rng2.random() - 0.5) * 0.04 * amount

def pose_walk(root, leg_L, leg_R, frame_i: int, n: int, species: str):
    t = frame_i / max(n, 1)
    phase = t * math.tau
    # bounce
    root.location.z = abs(math.sin(phase)) * 0.04
    root.rotation_euler.z = math.sin(phase) * 0.05
    if leg_L and leg_R:
        leg_L.rotation_euler.x = math.sin(phase) * 0.55
        leg_R.rotation_euler.x = math.sin(phase + math.pi) * 0.55
    if species == "slime_guide" and leg_L is None:
        # squash bounce on body child
        for c in root.children:
            if c.name == "body":
                s = 1.0 + math.sin(phase) * 0.08
                c.scale = (1.1 / s, 0.95 / s, 0.85 * s)


def main():
    args = parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    clear_scene()
    setup_world_camera(args.res)

    if args.species in ("traveler_m", "traveler_f"):
        root, leg_L, leg_R, head = build_traveler(args.species)
    elif args.species == "slime_guide":
        root, leg_L, leg_R, head = build_slime()
        leg_L = leg_R = None
    else:
        root, leg_L, leg_R, head = build_birdfolk()

    # face camera-ish: root faces -Y
    root.rotation_euler = Euler((0, 0, 0))

    frames_dir = out / args.species / "frames"
    frames_dir.mkdir(parents=True, exist_ok=True)
    paths = []
    # rest transforms for glitch reset
    rest = []
    for c in root.children:
        rest.append((c, c.scale.copy(), c.rotation_euler.copy(), c.location.copy()))

    for i in range(args.frames):
        for c, sc, rot, loc in rest:
            c.scale = sc.copy()
            c.rotation_euler = rot.copy()
            c.location = loc.copy()
        pose_walk(root, leg_L, leg_R, i, args.frames, args.species)
        apply_polygon_glitch(root, i, args.frames, args.glitch)
        bpy.context.view_layer.update()
        fp = frames_dir / f"walk_{i:02d}.png"
        bpy.context.scene.render.filepath = str(fp)
        bpy.ops.render.render(write_still=True)
        paths.append(fp)
        print("WROTE", fp)

    # simple horizontal sheet via blender compositing skip — stitch in py if PIL missing use bpy
    blend = out / args.species / f"{args.species}_v0.blend"
    bpy.ops.wm.save_as_mainfile(filepath=str(blend))
    print("BLEND", blend)
    sheet_path = out / args.species / f"{args.species}_walk_sheet.png"
    try:
        stitch_with_bpy(paths, sheet_path, args.res)
        print("SHEET", sheet_path)
    except Exception as e:
        print("SHEET_FAIL", e)


def stitch_with_bpy(paths, sheet_path: Path, res: int):
    """Load rendered frames and pack into one strip image."""
    import struct
    import zlib

    # minimal PNG reader/writer without PIL
    def read_png(path):
        data = Path(path).read_bytes()
        assert data[:8] == b"\x89PNG\r\n\x1a\n"
        pos = 8
        w = h = None
        idat = b""
        while pos < len(data):
            length = struct.unpack(">I", data[pos : pos + 4])[0]
            pos += 4
            ctype = data[pos : pos + 4]
            pos += 4
            chunk = data[pos : pos + length]
            pos += length
            pos += 4  # crc
            if ctype == b"IHDR":
                w, h = struct.unpack(">II", chunk[:8])
            elif ctype == b"IDAT":
                idat += chunk
            elif ctype == b"IEND":
                break
        raw = zlib.decompress(idat)
        rows = []
        stride = w * 4 + 1
        bpp = 4
        prev = bytearray(w * bpp)
        for y in range(h):
            row = raw[y * stride : (y + 1) * stride]
            ftype = row[0]
            data = bytearray(row[1:])
            if ftype == 0:  # None
                pass
            elif ftype == 1:  # Sub
                for i in range(len(data)):
                    left = data[i - bpp] if i >= bpp else 0
                    data[i] = (data[i] + left) & 0xFF
            elif ftype == 2:  # Up
                for i in range(len(data)):
                    data[i] = (data[i] + prev[i]) & 0xFF
            elif ftype == 3:  # Average
                for i in range(len(data)):
                    left = data[i - bpp] if i >= bpp else 0
                    data[i] = (data[i] + ((left + prev[i]) >> 1)) & 0xFF
            elif ftype == 4:  # Paeth
                def paeth(a, b, c):
                    p = a + b - c
                    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                    if pa <= pb and pa <= pc:
                        return a
                    if pb <= pc:
                        return b
                    return c
                for i in range(len(data)):
                    a = data[i - bpp] if i >= bpp else 0
                    b = prev[i]
                    c = prev[i - bpp] if i >= bpp else 0
                    data[i] = (data[i] + paeth(a, b, c)) & 0xFF
            else:
                raise ValueError(f"unsupported PNG filter {ftype}")
            prev = data
            rows.append(bytes(data))
        return w, h, rows

    def write_png(path, w, h, rows):
        def chunk(tag, data):
            return (
                struct.pack(">I", len(data))
                + tag
                + data
                + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
            )

        raw = b"".join(b"\x00" + r for r in rows)
        ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
        png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(
            b"IDAT", zlib.compress(raw, 9)
        ) + chunk(b"IEND", b"")
        Path(path).write_bytes(png)

    imgs = [read_png(p) for p in paths]
    w, h = imgs[0][0], imgs[0][1]
    n = len(imgs)
    sheet_w = w * n
    rows = []
    for y in range(h):
        row = b"".join(imgs[i][2][y] for i in range(n))
        rows.append(row)
    write_png(sheet_path, sheet_w, h, rows)


if __name__ == "__main__":
    main()
