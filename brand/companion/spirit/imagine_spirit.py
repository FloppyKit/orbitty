#!/usr/bin/env python3
"""Orbitty companion spirit — Grok Imagine 2.0 via pixel-artist OAuth.

Review only. Do not install into Orbitty.app. Do not farm Cursor.
"""
from __future__ import annotations

import base64
import json
import urllib.error
import urllib.request
from pathlib import Path

AUTH = Path.home() / ".hermes/profiles/pixel-artist/auth.json"
HERE = Path(__file__).resolve().parent
OUT = HERE / "raw"
MARK = HERE.parents[1] / "orbitty-pixel.png"

PORTRAIT = """ORIGINAL SNES-era JRPG talk-portrait pixel art, 3/4 camera, Teenage Engineering constraints.
Subject: a SPIRIT / wisp / breath-mass — the cream figure from the attached Orbitty mark
dissolved into air. Soft cream body-smoke, teal inner glow (the planet), one ochre ember.
Optional single eye-gleam. NO face kit. NO nose mouth ears hair clothes.
NOT a genie (no lamp, no turban, no smoke-from-bottle). NOT a cartoon bedsheet ghost.
NOT a human, NOT a town NPC, NOT a chibi, NOT a 3D render, NOT low-poly clay.
Crisp painted pixels, dark outline dyed to local hue, 2-3 shade ramps, readable when large.
SOLID MAGENTA background #FF00FF filling every non-sprite pixel. NO checkerboard. NO gray. NO text. NO watermark.
One figure, centered, 3/4 facing slightly right, floating, idle breath.
"""

STRIP = """ORIGINAL SNES-era JRPG pixel art idle cycle.
One horizontal strip, exactly 4 frames, equal cell size, 3/4 camera.
Same SPIRIT / wisp / cream breath-mass in every frame (Orbitty mark palette: cream, teal glow, ochre ember).
Frames only drift and pulse — wisps lift 1-2 pixels, ember ticks. Torso-mass stays put.
NOT a walk. NOT a genie. NOT a sheet ghost. NOT a person. NOT 3D.
SOLID MAGENTA background #FF00FF filling every non-sprite pixel. NO checkerboard. NO text. NO watermark.
"""


def token() -> str:
    return json.loads(AUTH.read_text())["credential_pool"]["xai-oauth"][0][
        "access_token"
    ]


def data_uri(path: Path) -> str:
    mime = "image/jpeg" if path.suffix.lower() in {".jpg", ".jpeg"} else "image/png"
    return f"data:{mime};base64,{base64.b64encode(path.read_bytes()).decode()}"


def edit(name: str, prompt: str, aspect: str, ref: Path) -> Path | None:
    OUT.mkdir(parents=True, exist_ok=True)
    dest = OUT / f"{name}.png"
    body = {
        "model": "grok-imagine-image-2.0",
        "prompt": prompt,
        "n": 1,
        "aspect_ratio": aspect,
        "resolution": "1k",
        "response_format": "b64_json",
        "image": {"url": data_uri(ref), "type": "image_url"},
    }
    req = urllib.request.Request(
        "https://api.x.ai/v1/images/edits",
        data=json.dumps(body).encode(),
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {token()}",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            payload = json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        err = e.read().decode(errors="replace")
        print(name, "HTTP", e.code, err[:600])
        return None
    except Exception as e:
        print(name, "FAIL", type(e).__name__, e)
        return None
    b64 = (payload.get("data") or [{}])[0].get("b64_json")
    if not b64:
        print(name, "no b64", list(payload.keys())[:8])
        return None
    dest.write_bytes(base64.b64decode(b64))
    print("ok", name, dest, dest.stat().st_size)
    return dest


def generate(name: str, prompt: str, aspect: str) -> Path | None:
    """Fallback: /images/generations (no source) if edits misbehaves."""
    OUT.mkdir(parents=True, exist_ok=True)
    dest = OUT / f"{name}.png"
    body = {
        "model": "grok-imagine-image-2.0",
        "prompt": prompt,
        "n": 1,
        "aspect_ratio": aspect,
        "resolution": "1k",
        "response_format": "b64_json",
    }
    req = urllib.request.Request(
        "https://api.x.ai/v1/images/generations",
        data=json.dumps(body).encode(),
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {token()}",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            payload = json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        err = e.read().decode(errors="replace")
        print(name, "gen HTTP", e.code, err[:600])
        return None
    except Exception as e:
        print(name, "gen FAIL", type(e).__name__, e)
        return None
    b64 = (payload.get("data") or [{}])[0].get("b64_json")
    if not b64:
        print(name, "gen no b64")
        return None
    dest.write_bytes(base64.b64decode(b64))
    print("ok-gen", name, dest, dest.stat().st_size)
    return dest


def main() -> None:
    if not MARK.exists():
        raise SystemExit(f"missing mark {MARK}")
    a = edit("spirit_portrait_v1", PORTRAIT, "2:3", MARK)
    if a is None:
        generate("spirit_portrait_v1", PORTRAIT, "2:3")
    b = edit("spirit_idle_strip_v1", STRIP, "16:9", MARK)
    if b is None:
        generate("spirit_idle_strip_v1", STRIP, "16:9")
    # second portrait roll
    edit("spirit_portrait_v2", PORTRAIT + "\nSlightly denser cream mass, one teal core, calmer.", "2:3", MARK)


if __name__ == "__main__":
    main()
