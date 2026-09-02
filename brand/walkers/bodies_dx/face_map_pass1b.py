"""
Pass 1b: Better face map — front-projected decal with clip, solid skin on back of head.
"""
from __future__ import annotations

import math
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/walkers/bodies_dx")
OUT = ROOT / "face_map_pass1"
TURNS = OUT / "turns_b"
TURNS.mkdir(exist_ok=True)

FACE = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/walkers/out/paperdoll_v0/ship_v0/heads/head_a_human_androgynous_v0.jpg")
BLEND = ROOT / "exports" / "femhumandx_waist_cinch.blend"


def setup():
    s = bpy.context.scene
    try:
        s.render.engine = "BLENDER_EEVEE_NEXT"
    except Exception:
        s.render.engine = "CYCLES"
        s.cycles.samples = 48
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
            bg.inputs[0].default_value = (0.05, 0.045, 0.07, 1)
            bg.inputs[1].default_value = 0.35
    except Exception:
        pass


def lights():
    for name, loc, e in [("K",(2.5,-2.2,2.4),140),("F",(-2.2,1.0,1.6),45),("R",(0,3.0,2.2),40)]:
        ld=bpy.data.lights.new(name, type="AREA"); ld.energy=e; ld.size=2.2
        o=bpy.data.objects.new(name, ld); o.location=loc
        bpy.context.scene.collection.objects.link(o)


def camera():
    cd=bpy.data.cameras.new("C"); cd.lens=60
    cam=bpy.data.objects.new("C", cd)
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera=cam
    return cam


def look_at(cam, target: Vector, dist, angle_deg=0.0, z_off=0.0):
    ang=math.radians(angle_deg)
    cam.location = target + Vector((math.sin(ang)*dist, -math.cos(ang)*dist, z_off))
    direction = target - cam.location
    cam.rotation_euler = direction.to_track_quat("-Z","Y").to_euler()


def head_center(head):
    coords=[head.matrix_world @ v.co for v in head.data.vertices]
    return sum(coords, Vector())/len(coords), max(c.z for c in coords)-min(c.z for c in coords)


def body_skin(obj, color=(0.78,0.55,0.62,1)):
    mat=bpy.data.materials.new(f"skin_{obj.name}")
    mat.use_nodes=True
    bsdf=mat.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs["Base Color"].default_value=color
        if "Roughness" in bsdf.inputs:
            bsdf.inputs["Roughness"].default_value=0.55
    obj.data.materials.clear()
    obj.data.materials.append(mat)


def face_decal_material(obj, face_path: Path, projector_cam):
    """Mix: skin base + face texture from camera projection (Generated/Object or UV project)."""
    img = bpy.data.images.load(str(face_path), check_existing=True)
    # Ensure UV project modifier for UV layer
    if not obj.data.uv_layers:
        obj.data.uv_layers.new(name="UVMap")
    # Clear old mods
    for m in list(obj.modifiers):
        obj.modifiers.remove(m)
    mod = obj.modifiers.new("UVProj", "UV_PROJECT")
    mod.projector_count = 1
    mod.projectors[0].object = projector_cam
    mod.uv_layer = obj.data.uv_layers[0].name

    mat = bpy.data.materials.new("face_decal")
    mat.use_nodes = True
    nt = mat.node_tree
    nodes, links = nt.nodes, nt.links
    nodes.clear()
    out = nodes.new("ShaderNodeOutputMaterial"); out.location=(500,0)
    bsdf = nodes.new("ShaderNodeBsdfPrincipled"); bsdf.location=(250,0)
    # skin color
    skin = nodes.new("ShaderNodeRGB"); skin.location=(-400,200)
    skin.outputs[0].default_value = (0.78, 0.55, 0.62, 1)
    # face tex
    tex = nodes.new("ShaderNodeTexImage"); tex.location=(-200,0)
    tex.image = img
    tex.interpolation = "Closest"
    tex.extension = "CLIP"  # critical: don't tile over skull
    uv = nodes.new("ShaderNodeUVMap"); uv.location=(-400,0)
    uv.uv_map = obj.data.uv_layers[0].name
    # mix face over skin where alpha/tex valid — use tex alpha; CLIP gives 0 outside
    mix = nodes.new("ShaderNodeMix"); mix.location=(50,0)
    mix.data_type = "RGBA"
    # factor from texture alpha
    links.new(uv.outputs["UV"], tex.inputs["Vector"])
    # If image has no alpha, use luminance of green-screen-ish / presence: use Math on alpha
    # JPG has no alpha — use a window: separate using map range on UV
    # Separate XYZ UV and check 0-1 range
    sep = nodes.new("ShaderNodeSeparateXYZ"); sep.location=(-200,250)
    links.new(uv.outputs["UV"], sep.inputs["Vector"])
    # in_bounds: u,v in 0..1 roughly
    # use compare nodes
    try:
        # Blender 4+ Mix node
        mix.blend_type = "MIX"
    except Exception:
        pass
    # Build factor with math
    math_u0 = nodes.new("ShaderNodeMath"); math_u0.operation="GREATER_THAN"; math_u0.inputs[1].default_value=0.02
    math_u1 = nodes.new("ShaderNodeMath"); math_u1.operation="LESS_THAN"; math_u1.inputs[1].default_value=0.98
    math_v0 = nodes.new("ShaderNodeMath"); math_v0.operation="GREATER_THAN"; math_v0.inputs[1].default_value=0.02
    math_v1 = nodes.new("ShaderNodeMath"); math_v1.operation="LESS_THAN"; math_v1.inputs[1].default_value=0.98
    links.new(sep.outputs["X"], math_u0.inputs[0])
    links.new(sep.outputs["X"], math_u1.inputs[0])
    links.new(sep.outputs["Y"], math_v0.inputs[0])
    links.new(sep.outputs["Y"], math_v1.inputs[0])
    mul1 = nodes.new("ShaderNodeMath"); mul1.operation="MULTIPLY"
    mul2 = nodes.new("ShaderNodeMath"); mul2.operation="MULTIPLY"
    mul3 = nodes.new("ShaderNodeMath"); mul3.operation="MULTIPLY"
    links.new(math_u0.outputs[0], mul1.inputs[0])
    links.new(math_u1.outputs[0], mul1.inputs[1])
    links.new(math_v0.outputs[0], mul2.inputs[0])
    links.new(math_v1.outputs[0], mul2.inputs[1])
    links.new(mul1.outputs[0], mul3.inputs[0])
    links.new(mul2.outputs[0], mul3.inputs[1])
    # Mix RGB
    # Blender 5 Mix node: Factor, A, B
    if "Factor" in mix.inputs:
        links.new(mul3.outputs[0], mix.inputs["Factor"])
    else:
        links.new(mul3.outputs[0], mix.inputs[0])
    # A = skin, B = face
    # inputs may be named A/B or Color1/Color2
    def mix_in(name_a, name_b, fallback_a, fallback_b):
        if name_a in mix.inputs:
            links.new(skin.outputs[0], mix.inputs[name_a])
            links.new(tex.outputs["Color"], mix.inputs[name_b])
        else:
            links.new(skin.outputs[0], mix.inputs[fallback_a])
            links.new(tex.outputs["Color"], mix.inputs[fallback_b])
    mix_in("A", "B", 6, 7)  # often socket indices for RGBA mix
    # Actually safer: use ShaderNodeMixRGB if exists
    # reconnect via MixRGB classic
    for n in list(nodes):
        if n != out and n != bsdf and n != tex and n != uv and n != skin and n != sep and n not in (math_u0,math_u1,math_v0,math_v1,mul1,mul2,mul3):
            if n.bl_idname == "ShaderNodeMix":
                nodes.remove(n)
    mixrgb = nodes.new("ShaderNodeMixRGB") if hasattr(bpy.types, "ShaderNodeMixRGB") else None
    if mixrgb is None:
        # create Mix
        mixrgb = nodes.new("ShaderNodeMix")
        mixrgb.data_type = "RGBA"
        mixrgb.location = (50,0)
        links.new(mul3.outputs[0], mixrgb.inputs["Factor"])
        # try sockets
        for a_name, b_name in [("A","B"), ("Color1","Color2")]:
            if a_name in mixrgb.inputs:
                links.new(skin.outputs[0], mixrgb.inputs[a_name])
                links.new(tex.outputs["Color"], mixrgb.inputs[b_name])
                break
        col_out = mixrgb.outputs.get("Result") or mixrgb.outputs[0]
        links.new(col_out, bsdf.inputs["Base Color"])
    else:
        mixrgb.location=(50,0)
        links.new(mul3.outputs[0], mixrgb.inputs["Fac"])
        links.new(skin.outputs[0], mixrgb.inputs["Color1"])
        links.new(tex.outputs["Color"], mixrgb.inputs["Color2"])
        links.new(mixrgb.outputs["Color"], bsdf.inputs["Base Color"])

    if "Roughness" in bsdf.inputs:
        bsdf.inputs["Roughness"].default_value = 0.6
    links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    obj.data.materials.clear()
    obj.data.materials.append(mat)
    return mat, mod


def apply_mod(obj, modname):
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    try:
        bpy.ops.object.modifier_apply(modifier=modname)
        print("applied", modname)
    except Exception as e:
        print("keep dynamic mod", e)


def main():
    bpy.ops.wm.open_mainfile(filepath=str(BLEND))
    setup()
    lights()
    cam = camera()

    head=None; body=None
    for o in bpy.data.objects:
        if o.type!="MESH": continue
        n=o.name.lower()
        if "body" in n: body=o
        elif n.startswith("head") or "furry" in n:
            if head is None or len(o.data.vertices)>len(head.data.vertices):
                head=o
    print("head", head.name, "verts", len(head.data.vertices))
    if body: body_skin(body)
    for o in bpy.data.objects:
        if o.type=="MESH" and o not in (head, body):
            body_skin(o)

    center, h = head_center(head)
    # Projection camera dead front of face (mesh faces -Y typically)
    proj = camera()
    proj.name = "ProjCam"
    # place projector in front of face
    # estimate front from head bbox y
    ys=[(head.matrix_world @ v.co).y for v in head.data.vertices]
    zs=[(head.matrix_world @ v.co).z for v in head.data.vertices]
    xs=[(head.matrix_world @ v.co).x for v in head.data.vertices]
    front = Vector((sum(xs)/len(xs), min(ys)-0.45, sum(zs)/len(zs)))
    proj.location = front
    look_at(proj, Vector((sum(xs)/len(xs), sum(ys)/len(ys), sum(zs)/len(zs))), 0.01, 0)  # will overwrite
    # aim projector at head center
    direction = center - proj.location
    proj.rotation_euler = direction.to_track_quat("-Z","Y").to_euler()
    proj.data.lens = 35  # wider to cover face

    mat, mod = face_decal_material(head, FACE, proj)
    apply_mod(head, mod.name)

    # Save
    bpy.ops.wm.save_as_mainfile(filepath=str(OUT/"femhuman_face_map_pass1b.blend"))

    # Render shots with view cam
    look_at(cam, center, dist=max(0.7, h*2.6), angle_deg=0, z_off=0)
    for name, ang, dist_mul, z_off in [
        ("portrait_front", 0, 2.6, 0),
        ("portrait_34", 32, 2.6, 0),
        ("portrait_profile", 85, 2.7, 0),
        ("full_front", 0, None, None),
        ("full_34", 40, None, None),
    ]:
        if name.startswith("full"):
            # full body
            coords=[]
            for o in bpy.data.objects:
                if o.type!="MESH": continue
                for v in o.data.vertices:
                    coords.append(o.matrix_world@v.co)
            c=sum(coords,Vector())/len(coords)
            hh=max(c.z for c in coords)-min(c.z for c in coords) if False else max(p.z for p in coords)-min(p.z for p in coords)
            look_at(cam, c, dist=max(3.2, hh*1.9), angle_deg=ang, z_off=0)
        else:
            look_at(cam, center, dist=max(0.65, h*dist_mul), angle_deg=ang, z_off=z_off)
        path=TURNS/f"{name}.png"
        bpy.context.scene.render.filepath=str(path)
        bpy.ops.render.render(write_still=True)
        print("R", name)

    for ang in [0,45,90,135,180]:
        coords=[]
        for o in bpy.data.objects:
            if o.type!="MESH": continue
            for v in o.data.vertices:
                coords.append(o.matrix_world@v.co)
        c=sum(coords,Vector())/len(coords)
        hh=max(p.z for p in coords)-min(p.z for p in coords)
        look_at(cam, c, dist=max(3.2, hh*1.9), angle_deg=ang)
        bpy.context.scene.render.filepath=str(TURNS/f"turn_{ang:03d}.png")
        bpy.ops.render.render(write_still=True)
    print("PASS1B_OK")

main()
