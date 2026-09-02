"""Jacob fidelity bake — FBX only, never opens Jacob.blend.

512×768, 12 frames, ink hull, AO/bump folds, harder light.
CLI: blender -b -Y --factory-startup --python-exit-code 1 -P this.py
"""
from __future__ import annotations

import json
import math
from pathlib import Path

import bpy
from mathutils import Vector

SRC = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion/jacob_src")
FBX = SRC / "Jacob.fbx"
TEX = SRC / "textures"
OUT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion/jacob_idle_bake")
FRAMES = OUT / "frames_hi"
STRIP = OUT / "preview_strip_hi.png"
REPORT = OUT / "inspect_hi.json"
W, H, N = 512, 768, 12
HIDE_EXACT = {"Grass", "Ground", "Rocks"}
HIDE_SUBSTR = ("grass", "ground", "rock", "scene")


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def setup_render():
    s = bpy.context.scene
    try:
        s.render.engine = "BLENDER_EEVEE_NEXT"
    except Exception:
        try:
            s.render.engine = "BLENDER_EEVEE"
        except Exception:
            s.render.engine = "CYCLES"
            s.cycles.samples = 32
    s.render.resolution_x = W
    s.render.resolution_y = H
    s.render.resolution_percentage = 100
    s.render.film_transparent = True
    s.render.image_settings.file_format = "PNG"
    s.render.image_settings.color_mode = "RGBA"
    s.render.image_settings.color_depth = "8"
    s.render.filter_size = 1.2
    try:
        s.view_settings.view_transform = "Standard"
    except Exception:
        pass
    try:
        s.view_settings.look = "None"
        s.view_settings.exposure = 0.05
        s.view_settings.gamma = 1.0
    except Exception:
        pass
    if not s.world:
        s.world = bpy.data.worlds.new("W")
    try:
        s.world.use_nodes = True
        bg = s.world.node_tree.nodes.get("Background")
        if bg:
            bg.inputs[0].default_value = (0.42, 0.50, 0.60, 1)
            bg.inputs[1].default_value = 0.18
    except Exception:
        pass
    # Harder key + rim so clothing folds read; fill stays dim.
    for name, loc, energy, size in [
        ("Key", (2.4, -3.2, 3.8), 560, 1.5),
        ("Fill", (-3.2, -1.2, 2.0), 55, 3.2),
        ("Rim", (0.6, 3.8, 3.0), 280, 1.2),
        ("Front", (0.0, -4.6, 1.6), 70, 2.0),
    ]:
        ld = bpy.data.lights.new(name, type="AREA")
        ld.energy = energy
        ld.size = size
        o = bpy.data.objects.new(name, ld)
        bpy.context.collection.objects.link(o)
        o.location = loc
        o.rotation_euler = (
            math.radians(52),
            0,
            math.atan2(loc[0], -loc[1]) if loc[1] else 0,
        )


def import_fbx():
    bpy.ops.import_scene.fbx(
        filepath=str(FBX),
        automatic_bone_orientation=False,
        use_anim=True,
        ignore_leaf_bones=False,
        anim_offset=0.0,
    )


def hide_set():
    hidden = []
    for o in list(bpy.data.objects):
        n = o.name
        low = n.lower()
        if n in HIDE_EXACT or any(s in low for s in HIDE_SUBSTR) or "outline" in low:
            o.hide_render = True
            o.hide_viewport = True
            try:
                o.hide_set(True)
            except Exception:
                pass
            hidden.append(n)
    return hidden


def ink_material():
    """Shadeless black on backfaces only — inverted-hull silhouette."""
    mat = bpy.data.materials.new("OrbittyInk")
    mat.use_nodes = True
    ntree = mat.node_tree
    ntree.nodes.clear()
    out = ntree.nodes.new("ShaderNodeOutputMaterial")
    out.location = (400, 0)
    mix = ntree.nodes.new("ShaderNodeMixShader")
    mix.location = (180, 0)
    trans = ntree.nodes.new("ShaderNodeBsdfTransparent")
    trans.location = (0, 80)
    emit = ntree.nodes.new("ShaderNodeEmission")
    emit.location = (0, -80)
    emit.inputs["Color"].default_value = (0.01, 0.01, 0.02, 1)
    emit.inputs["Strength"].default_value = 1.0
    geom = ntree.nodes.new("ShaderNodeNewGeometry")
    geom.location = (-200, 0)
    ntree.links.new(geom.outputs["Backfacing"], mix.inputs["Fac"])
    ntree.links.new(trans.outputs["BSDF"], mix.inputs[1])
    ntree.links.new(emit.outputs["Emission"], mix.inputs[2])
    ntree.links.new(mix.outputs["Shader"], out.inputs["Surface"])
    try:
        mat.blend_method = "BLEND"
    except Exception:
        pass
    try:
        mat.shadow_method = "NONE"
    except Exception:
        pass
    try:
        mat.use_backface_culling = False
    except Exception:
        pass
    return mat


def add_ink_hull(ink_mat, thickness=0.007):
    """Inverted-hull on a *copy* — never touch the colored mesh's slots."""
    tagged = []
    originals = [
        o
        for o in list(bpy.data.objects)
        if o.type == "MESH"
        and not o.hide_render
        and "outline" not in o.name.lower()
        and not any(s in o.name.lower() for s in HIDE_SUBSTR)
        and not o.name.startswith("Ink_")
    ]
    for o in originals:
        hull = o.copy()
        hull.data = o.data.copy()
        hull.name = f"Ink_{o.name}"[:60]
        hull.data.materials.clear()
        hull.data.materials.append(ink_mat)
        bpy.context.collection.objects.link(hull)
        if o.parent:
            hull.parent = o.parent
            hull.matrix_parent_inverse = o.matrix_parent_inverse.copy()
        hull.matrix_world = o.matrix_world.copy()
        try:
            hull.visible_shadow = False
        except Exception:
            pass
        try:
            hull.visible_volume_scatter = False
        except Exception:
            pass
        mod = hull.modifiers.new("orbitty_ink", "SOLIDIFY")
        mod.thickness = thickness
        try:
            mod.offset = 1.0
        except Exception:
            pass
        try:
            mod.use_flip_normals = True
        except Exception:
            pass
        try:
            mod.use_rim = False
        except Exception:
            pass
        tagged.append(hull.name)
    return tagged


def _mix_node(ntree):
    try:
        n = ntree.nodes.new("ShaderNodeMix")
        n.data_type = "RGBA"
        try:
            n.blend_type = "MULTIPLY"
        except Exception:
            pass
        return n, "A", "B", "Result"
    except Exception:
        n = ntree.nodes.new("ShaderNodeMixRGB")
        n.blend_type = "MULTIPLY"
        return n, "Color1", "Color2", "Color"


def remap_textures():
    color = TEX / "T_JacobColor.png"
    props = TEX / "T_PropsAtlas.png"
    if not color.exists():
        return
    img = bpy.data.images.load(str(color), check_existing=True)
    prop_img = bpy.data.images.load(str(props), check_existing=True) if props.exists() else None
    for mat in bpy.data.materials:
        if mat.name == "OrbittyInk":
            continue
        if not mat.use_nodes:
            continue
        ntree = mat.node_tree
        name = mat.name.lower()
        if "scene" in name:
            continue
        target = prop_img if ("prop" in name or "atlas" in name) and prop_img else img
        tex = None
        for n in ntree.nodes:
            if n.type == "TEX_IMAGE":
                tex = n
                break
        if tex is None:
            tex = ntree.nodes.new("ShaderNodeTexImage")
            tex.location = (-500, 220)
        tex.image = target
        bsdf = ntree.nodes.get("Principled BSDF")
        if not bsdf:
            continue
        for link in list(ntree.links):
            if link.to_node == bsdf and link.to_socket.name == "Base Color":
                ntree.links.remove(link)
        ntree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
        bump = ntree.nodes.new("ShaderNodeBump")
        bump.location = (-220, -40)
        bump.inputs["Strength"].default_value = 0.22
        bump.inputs["Distance"].default_value = 0.05
        ntree.links.new(tex.outputs["Color"], bump.inputs["Height"])
        ntree.links.new(bump.outputs["Normal"], bsdf.inputs["Normal"])
        if "Specular IOR Level" in bsdf.inputs:
            bsdf.inputs["Specular IOR Level"].default_value = 0.08
        if "Roughness" in bsdf.inputs:
            bsdf.inputs["Roughness"].default_value = 0.55


def action_curve_count(act) -> int:
    try:
        n = len(getattr(act, "fcurves", []) or [])
        if n:
            return n
    except Exception:
        pass
    n = 0
    try:
        for layer in getattr(act, "layers", []) or []:
            for strip in getattr(layer, "strips", []) or []:
                bags = []
                bag = getattr(strip, "channelbag", None)
                if callable(bag):
                    try:
                        bag = bag()
                    except Exception:
                        bag = None
                if bag:
                    bags.append(bag)
                extra = getattr(strip, "channelbags", None)
                if extra:
                    try:
                        bags.extend(list(extra))
                    except Exception:
                        pass
                for b in bags:
                    try:
                        n += len(getattr(b, "fcurves", []) or [])
                    except Exception:
                        pass
    except Exception:
        pass
    return n


def inspect():
    armatures = [o.name for o in bpy.data.objects if o.type == "ARMATURE"]
    meshes = [
        {
            "name": o.name,
            "hide_render": bool(o.hide_render),
            "verts": len(o.data.vertices) if o.data else 0,
        }
        for o in bpy.data.objects
        if o.type == "MESH"
    ]
    actions = []
    for a in bpy.data.actions:
        f0 = f1 = None
        try:
            if a.frame_range:
                f0, f1 = float(a.frame_range[0]), float(a.frame_range[1])
        except Exception:
            pass
        actions.append(
            {
                "name": a.name,
                "frame_start": f0,
                "frame_end": f1,
                "fcurves": action_curve_count(a),
            }
        )
    info = {
        "armatures": armatures,
        "meshes": meshes,
        "actions": actions,
        "materials": [m.name for m in bpy.data.materials],
        "blender": bpy.app.version_string,
        "opened_blend": False,
        "source": str(FBX),
        "res": [W, H, N],
    }
    REPORT.write_text(json.dumps(info, indent=2))
    return info, next((o for o in bpy.data.objects if o.type == "ARMATURE"), None)


def pick_idle(actions):
    names = [a["name"] for a in actions]
    for n in names:
        if n.lower().endswith("|idle") or n.lower() == "idle":
            return n
    for n in names:
        if "idle" in n.lower() and "grass" not in n.lower() and "ground" not in n.lower():
            return n
    return names[0] if names else None


def character_meshes(for_camera=False):
    out = []
    for o in bpy.data.objects:
        if o.type != "MESH" or o.hide_render:
            continue
        low = o.name.lower()
        if any(s in low for s in HIDE_SUBSTR) or "outline" in low or o.name.startswith("Ink_"):
            continue
        if for_camera and ("claymore" in low or "sword" in low):
            continue
        out.append(o)
    return out


def mesh_world_bounds(objs):
    mins = Vector((1e9, 1e9, 1e9))
    maxs = Vector((-1e9, -1e9, -1e9))
    deps = bpy.context.evaluated_depsgraph_get()
    hit = False
    for o in objs:
        ev = o.evaluated_get(deps)
        mesh = ev.to_mesh()
        try:
            for v in mesh.vertices:
                w = ev.matrix_world @ v.co
                mins.x = min(mins.x, w.x)
                mins.y = min(mins.y, w.y)
                mins.z = min(mins.z, w.z)
                maxs.x = max(maxs.x, w.x)
                maxs.y = max(maxs.y, w.y)
                maxs.z = max(maxs.z, w.z)
                hit = True
        finally:
            ev.to_mesh_clear()
    if not hit:
        raise RuntimeError("no character verts for camera")
    return mins, maxs


def fit_camera():
    meshes = character_meshes(for_camera=True)
    mins, maxs = mesh_world_bounds(meshes)
    center = (mins + maxs) * 0.5
    size = maxs - mins
    height = max(0.05, size.z)
    width = max(size.x, size.y, 0.05)
    cam_d = bpy.data.cameras.new("Cam")
    cam_d.lens = 70
    cam_d.clip_start = 0.01
    cam_d.clip_end = 200
    cam_d.sensor_width = 36
    cam = bpy.data.objects.new("Cam", cam_d)
    bpy.context.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    dist = max(height * 2.55, width * 3.15)
    look = Vector((center.x, center.y, mins.z + height * 0.48))
    cam.location = Vector((look.x + dist * 0.28, look.y - dist * 1.02, look.z + height * 0.06))
    direction = look - cam.location
    cam.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()
    return {
        "bounds_min": list(mins),
        "bounds_max": list(maxs),
        "height": height,
        "width": width,
        "mesh_names": [o.name for o in meshes],
        "cam": list(cam.location),
        "look": list(look),
        "dist": dist,
    }


def assign_action(arm, name):
    act = bpy.data.actions.get(name)
    if not arm or not act:
        return None
    if not arm.animation_data:
        arm.animation_data_create()
    ad = arm.animation_data
    assigned = {"legacy": False, "slot": None, "nla": False}
    try:
        ad.action = act
        assigned["legacy"] = True
    except Exception as e:
        assigned["legacy_error"] = str(e)
    try:
        slots = list(getattr(act, "slots", []) or [])
        if slots and hasattr(ad, "action_slot"):
            chosen = slots[0]
            for s in slots:
                sn = (getattr(s, "name_display", None) or getattr(s, "name", "") or "").lower()
                if "armature" in sn or "object" in sn:
                    chosen = s
                    break
            ad.action_slot = chosen
            assigned["slot"] = getattr(chosen, "name_display", None) or chosen.name
    except Exception as e:
        assigned["slot_error"] = str(e)
    try:
        while ad.nla_tracks:
            ad.nla_tracks.remove(ad.nla_tracks[0])
        track = ad.nla_tracks.new()
        track.name = "idle_bake"
        f0, f1 = act.frame_range
        strip = track.strips.new("idle", int(f0), act)
        strip.action = act
        try:
            if hasattr(ad, "action_slot") and ad.action_slot and hasattr(strip, "action_slot"):
                strip.action_slot = ad.action_slot
        except Exception:
            pass
        strip.frame_start = float(f0)
        strip.frame_end = float(f1)
        ad.use_nla = True
        assigned["nla"] = True
    except Exception as e:
        assigned["nla_error"] = str(e)
    f0, f1 = act.frame_range
    bpy.context.scene.frame_start = int(f0)
    bpy.context.scene.frame_end = int(f1)
    bpy.context.scene.frame_set(int(f0))
    return float(f0), float(f1), assigned


def bake(f0, f1):
    FRAMES.mkdir(parents=True, exist_ok=True)
    for p in FRAMES.glob("idle_*.png"):
        p.unlink()
    if STRIP.exists():
        STRIP.unlink()
    span = max(1.0, f1 - f0)
    paths = []
    s = bpy.context.scene
    for i in range(N):
        t = f0 + (i / N) * span
        s.frame_set(int(round(t)))
        out = FRAMES / f"idle_{i}.png"
        s.render.filepath = str(out.with_suffix(""))
        bpy.ops.render.render(write_still=True)
        if not out.exists():
            candidates = list(FRAMES.glob(f"idle_{i}*.png"))
            if candidates:
                candidates[0].replace(out)
        paths.append(str(out))
        print("FRAME", i, "t", t, "exists", out.exists(), "bytes", out.stat().st_size if out.exists() else 0)
    return paths


def write_strip(paths):
    imgs = []
    for p in paths:
        path = Path(p)
        if not path.exists():
            continue
        imgs.append(bpy.data.images.load(str(path)))
    if not imgs:
        return None
    w, h = imgs[0].size
    strip_w = w * len(imgs)
    out = bpy.data.images.new("preview_strip_hi", width=strip_w, height=h, alpha=True)
    pixels = [0.0] * (strip_w * h * 4)
    for i, im in enumerate(imgs):
        src = list(im.pixels)
        for y in range(h):
            row = y * w * 4
            drow = y * strip_w * 4 + i * w * 4
            pixels[drow : drow + w * 4] = src[row : row + w * 4]
    out.pixels = pixels
    out.filepath_raw = str(STRIP)
    out.file_format = "PNG"
    out.save()
    return str(STRIP)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    if not FBX.exists():
        raise SystemExit(f"missing FBX {FBX}")
    reset()
    setup_render()
    import_fbx()
    hidden = hide_set()
    remap_textures()
    info, arm = inspect()
    info["hidden"] = hidden
    info["inked"] = []
    info["ink"] = "post_png"
    idle = pick_idle(info["actions"])
    info["picked_idle"] = idle
    if not idle:
        REPORT.write_text(json.dumps(info, indent=2))
        raise SystemExit("no idle action")
    fr = assign_action(arm, idle)
    info["idle_assign"] = fr[2] if fr else None
    info["idle_range"] = [fr[0], fr[1]] if fr else None
    info["camera"] = fit_camera()
    REPORT.write_text(json.dumps(info, indent=2))
    if not fr:
        raise SystemExit("idle assign failed")
    paths = bake(fr[0], fr[1])
    info["baked"] = paths
    try:
        info["strip"] = write_strip(paths)
    except Exception as e:
        info["strip_error"] = str(e)
    REPORT.write_text(json.dumps(info, indent=2))
    print("BAKE_HI_OK", idle, fr[0], fr[1], "frames", N, "res", W, H)


if __name__ == "__main__":
    main()
