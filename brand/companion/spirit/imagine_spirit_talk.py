#!/usr/bin/env python3
"""Talk variants only — not a bless, not an install."""
from __future__ import annotations

import base64
import json
import urllib.error
import urllib.request
from pathlib import Path

AUTH = Path.home() / ".hermes/profiles/pixel-artist/auth.json"
OUT = Path(
    "/Users/dude/goose artifacts/livesprite-tauri/brand/companion/spirit/raw"
)
SRC = OUT / "spirit_portrait_v3.png"

BRIGHT = """Same exact spirit as attached: cream breath-mass, teal diamond core, ochre/orange square ember, one eye-gleam, 3/4 JRPG talk-portrait.
PUSH SNES vibrancy: brighter cream (almost warm paper), electric teal core like a mana crystal, hotter ochre square. More light, still pixel art.
Keep the orange square. Keep no face kit. NOT a genie, NOT a person, NOT 3D, NOT a bedsheet ghost.
SOLID MAGENTA background #FF00FF. NO checkerboard. NO text. NO watermark.
"""

CHUNKY = """Same spirit as attached (cream wisp, teal core, KEEP the orange square, one eye).
CHUNKIER 16-bit SNES talk-portrait: fewer, larger pixels, thicker dyed outlines, 3-shade ramps only. Read as a cartridge portrait, not a photo-filter.
3/4 camera. NOT a genie. NOT a human. NOT 3D.
SOLID MAGENTA background #FF00FF. NO checkerboard. NO text.
"""


def token() -> str:
    return json.loads(AUTH.read_text())["credential_pool"]["xai-oauth"][0][
        "access_token"
    ]


def edit(name: str, prompt: str) -> Path | None:
    dest = OUT / f"{name}.png"
    body = {
        "model": "grok-imagine-image-2.0",
        "prompt": prompt,
        "n": 1,
        "aspect_ratio": "2:3",
        "resolution": "1k",
        "response_format": "b64_json",
        "image": {
            "url": f"data:image/png;base64,{base64.b64encode(SRC.read_bytes()).decode()}",
            "type": "image_url",
        },
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
        print(name, "HTTP", e.code, e.read().decode(errors="replace")[:500])
        return None
    except Exception as e:
        print(name, type(e).__name__, e)
        return None
    b64 = (payload.get("data") or [{}])[0].get("b64_json")
    if not b64:
        print(name, "no b64")
        return None
    dest.write_bytes(base64.b64decode(b64))
    print("ok", dest, dest.stat().st_size)
    return dest


if __name__ == "__main__":
    edit("spirit_portrait_v4_bright", BRIGHT)
    edit("spirit_portrait_v5_chunky", CHUNKY)
