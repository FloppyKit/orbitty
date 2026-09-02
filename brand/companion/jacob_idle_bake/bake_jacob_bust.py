"""GO-ORBITTY-COMPANION-HEADROOM — Jacob temp BUST camera (proof only).

Second camera over the same blessed idle bake: head + neck + collar hint,
256×384 RGBA, same 8 time samples as bake_jacob_idle.py. Output goes to
temp_bodies/companion-jacob/bust_*.png (the pack contract's bust set).

Proof only: Jacob stays a temp body, never the default, never CoS.

Safety (same as bake_jacob_idle.py):
- NEVER opens Jacob.blend (script risk lives there).
- CLI must pass -Y / --disable-autoexec.
- Imports Jacob.fbx + textures only.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import bpy
from mathutils import Vector

sys.path.insert(0, str(Path(__file__).parent))
import bake_jacob_idle as base  # noqa: E402

OUT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion/temp_bodies/companion-jacob")
REPORT = Path("/Users/dude/goose artifacts/livesprite-tauri/brand/companion/jacob_idle_bake/inspect_bust.json")
N = 8
# Bust region: top slice of the figure — head + neck + a collar hint.
REGION = 0.26


def fit_bust_camera():
    meshes = base.character_meshes(for_camera=True)
    mins, maxs = base.mesh_world_bounds(meshes)
    center = (mins + maxs) * 0.5
    height = max(0.05, maxs.z - mins.z)
    region = height * REGION
    cam_d = bpy.data.cameras.new("BustCam")
    cam_d.lens = 70
    cam_d.clip_start = 0.01
    cam_d.clip_end = 200
    cam_d.sensor_width = 36
    cam = bpy.data.objects.new("BustCam", cam_d)
    bpy.context.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    # Same push-in rule as the full-body fit (region * 2.55), same slight
    # side angle, aimed at the head/neck band under the crown.
    dist = region * 2.55
    look = Vector((center.x, center.y, maxs.z - region * 0.52))
    cam.location = Vector((look.x + dist * 0.28, look.y - dist * 1.02, look.z + region * 0.06))
    direction = look - cam.location
    cam.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()
    return {
        "bounds_min": list(mins),
        "bounds_max": list(maxs),
        "height": height,
        "region": region,
        "cam": list(cam.location),
        "look": list(look),
        "dist": dist,
        "lens": 70,
    }


def bake_bust(f0, f1):
    OUT.mkdir(parents=True, exist_ok=True)
    for p in OUT.glob("bust_*.png"):
        p.unlink()
    span = max(1.0, f1 - f0)
    paths = []
    s = bpy.context.scene
    for i in range(N):
        t = f0 + (i / N) * span
        s.frame_set(int(round(t)))
        out = OUT / f"bust_{i}.png"
        s.render.filepath = str(out.with_suffix(""))
        bpy.ops.render.render(write_still=True)
        if not out.exists():
            candidates = list(OUT.glob(f"bust_{i}*.png"))
            if candidates:
                candidates[0].replace(out)
        paths.append(str(out))
        print("BUST", i, "t", t, "exists", out.exists(), "bytes", out.stat().st_size if out.exists() else 0)
    return paths


def main():
    if not base.FBX.exists():
        raise SystemExit(f"missing FBX {base.FBX}")
    base.reset()
    base.setup_render()
    base.import_fbx()
    hidden = base.hide_set()
    base.remap_textures()
    info, arm = base.inspect()
    info["hidden"] = hidden
    idle = base.pick_idle(info["actions"])
    info["picked_idle"] = idle
    if not idle:
        raise SystemExit("no idle action")
    fr = base.assign_action(arm, idle)
    info["idle_assign"] = fr[2] if fr else None
    info["idle_range"] = [fr[0], fr[1]] if fr else None
    info["bust_camera"] = fit_bust_camera()
    if not fr:
        REPORT.write_text(json.dumps(info, indent=2))
        raise SystemExit("idle assign failed")
    paths = bake_bust(fr[0], fr[1])
    info["baked_bust"] = paths
    REPORT.write_text(json.dumps(info, indent=2))
    print("BUST_BAKE_OK", idle, fr[0], fr[1])


if __name__ == "__main__":
    main()
