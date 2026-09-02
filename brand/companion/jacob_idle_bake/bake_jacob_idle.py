"""Bake Jacob FBX Idle → 8×256×384 PNG.

Safety:
- NEVER opens Jacob.blend (script risk lives there).
- CLI must pass -Y / --disable-autoexec.
- Imports Jacob.fbx + textures only.
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
FRAMES = OUT / "frames"
STRIP = OUT / "preview_strip.png"
W, H, N = 256, 384, 8
REPORT = OUT / "inspect.json"
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
            s.cycles.samples = 24
    s.render.resolution_x = W
    s.render.resolution_y = H
    s.render.resolution_percentage = 100
    s.render.film_transparent = True
    s.render.image_settings.file_format = "PNG"
    s.render.image_settings.color_mode = "RGBA"
    s.render.image_settings.color_depth = "8"
    s.render.filepath = str(FRAMES / "idle_")
    try:
        s.view_settings.view_transform = "Standard"
    except Exception:
        pass
    try:
        s.view_settings.look = "None"
    except Exception:
        pass
    try:
        s.view_settings.exposure = 0.15
        s.view_settings.gamma = 1.0
    except Exception:
        pass
    if not s.world:
        s.world = bpy.data.worlds.new("W")
    try:
        s.world.use_nodes = True
        bg = s.world.node_tree.nodes.get("Background")
        if bg:
            bg.inputs[0].default_value = (0.55, 0.64, 0.74, 1)
            bg.inputs[1].default_value = 0.28
    except Exception:
        pass
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
            math.radians(50),
            0,
            math.atan2(loc[0], -loc[1]) if loc[1] else 0,
        )


def import_fbx():
    # Keep author bone axes — auto-orient can kill the Idle clip.
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
        if n in HIDE_EXACT or any(s in low for s in HIDE_SUBSTR):
            o.hide_render = True
            o.hide_viewport = True
            o.hide_set(True)
            hidden.append(n)
    return hidden


def remap_textures():
    color = TEX / "T_JacobColor.png"
    props = TEX / "T_PropsAtlas.png"
    if not color.exists():
        return
    img = bpy.data.images.load(str(color), check_existing=True)
    prop_img = bpy.data.images.load(str(props), check_existing=True) if props.exists() else None
    for mat in bpy.data.materials:
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
            tex.location = (-400, 200)
        tex.image = target
        bsdf = ntree.nodes.get("Principled BSDF")
        if bsdf:
            linked = any(
                l.to_node == bsdf and l.to_socket.name == "Base Color" for l in ntree.links
            )
            if not linked:
                ntree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
            if "Specular IOR Level" in bsdf.inputs:
                bsdf.inputs["Specular IOR Level"].default_value = 0.12
            if "Roughness" in bsdf.inputs:
                bsdf.inputs["Roughness"].default_value = 0.62


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
        slots = []
        try:
            slots = [getattr(s, "name_display", None) or s.name for s in getattr(a, "slots", []) or []]
        except Exception:
            slots = []
        actions.append(
            {
                "name": a.name,
                "frame_start": f0,
                "frame_end": f1,
                "fcurves": action_curve_count(a),
                "slots": slots,
            }
        )
    bones = []
    arm = next((o for o in bpy.data.objects if o.type == "ARMATURE"), None)
    if arm:
        bones = [b.name for b in arm.data.bones][:40]
    info = {
        "armatures": armatures,
        "meshes": meshes,
        "actions": actions,
        "bone_sample": bones,
        "materials": [m.name for m in bpy.data.materials],
        "images": [i.name for i in bpy.data.images],
        "blender": bpy.app.version_string,
        "opened_blend": False,
        "source": str(FBX),
    }
    REPORT.write_text(json.dumps(info, indent=2))
    return info, arm


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
        if any(s in low for s in HIDE_SUBSTR):
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
    # Full body in 2:3. Sword may peek the top; head/feet must stay in.
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
        "lens": 70,
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
    # Blender 5 action slots
    try:
        slots = list(getattr(act, "slots", []) or [])
        if slots and hasattr(ad, "action_slot"):
            chosen = None
            for s in slots:
                sn = (getattr(s, "name_display", None) or getattr(s, "name", "") or "").lower()
                if "armature" in sn or sn.endswith("idle") or "object" in sn:
                    chosen = s
                    break
            if chosen is None:
                chosen = slots[0]
            ad.action_slot = chosen
            assigned["slot"] = getattr(chosen, "name_display", None) or chosen.name
    except Exception as e:
        assigned["slot_error"] = str(e)
    # NLA fallback so the clip actually evaluates
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
        assigned["nla_range"] = [float(f0), float(f1)]
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
        rendered = Path(str(out) + ".png") if not out.exists() else out
        # Blender writes filepath+".png"; we passed idle_0 so expect idle_0.png
        if not out.exists():
            # try whatever was written
            candidates = list(FRAMES.glob(f"idle_{i}*.png"))
            if candidates:
                candidates[0].replace(out)
        paths.append(str(out))
        print("FRAME", i, "t", t, "exists", out.exists(), "bytes", out.stat().st_size if out.exists() else 0)
    return paths


def write_strip(paths):
    # Assemble 8-wide strip with Blender images (no extra deps).
    imgs = []
    for p in paths:
        path = Path(p)
        if not path.exists():
            continue
        im = bpy.data.images.load(str(path))
        imgs.append(im)
    if not imgs:
        return None
    w, h = imgs[0].size
    strip_w = w * len(imgs)
    out = bpy.data.images.new("preview_strip", width=strip_w, height=h, alpha=True)
    pixels = [0.0] * (strip_w * h * 4)
    for i, im in enumerate(imgs):
        src = list(im.pixels)
        for y in range(h):
            for x in range(w):
                si = (y * w + x) * 4
                di = (y * strip_w + i * w + x) * 4
                pixels[di : di + 4] = src[si : si + 4]
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
    idle = pick_idle(info["actions"])
    info["picked_idle"] = idle
    if not idle:
        REPORT.write_text(json.dumps(info, indent=2))
        raise SystemExit("no idle action")
    fr = assign_action(arm, idle)
    info["idle_assign"] = fr[2] if fr else None
    info["idle_range"] = [fr[0], fr[1]] if fr else None
    cam_info = fit_camera()
    info["camera"] = cam_info
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
    print("BAKE_OK", idle, fr[0], fr[1])


if __name__ == "__main__":
    main()
