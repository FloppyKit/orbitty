#!/usr/bin/env python3
"""Second Imagine pass: v2 spirit as source (the actual wisp)."""
from __future__ import annotations

import base64
import json
import urllib.error
import urllib.request
from pathlib import Path

AUTH = Path.home() / ".hermes/profiles/pixel-artist/auth.json"
OUT = Path(__file__).resolve().parent / "raw"
SRC = OUT / "spirit_portrait_v2.png"

STRIP = """Keep THIS exact spirit: cream pixel-smoke body, teal diamond core, ochre square ember, 3/4 JRPG talk-portrait, no face kit.
One horizontal strip, exactly 4 idle frames, equal cells.
Only the wisps drift 1-2 pixels and the ember ticks. Same silhouette. NOT a walk. NOT a person. NOT a genie.
SOLID MAGENTA background #FF00FF on every non-sprite pixel. NO checkerboard. NO text. NO watermark.
"""

PORTRAIT = """Same spirit as the attached: cream breath-mass, teal core, ochre ember, 3/4 camera, SNES JRPG painted pixels.
One figure, larger in frame, optional single eye-gleam, trailing wisps.
NOT a human, NOT a genie, NOT a bedsheet ghost, NOT 3D.
SOLID MAGENTA background #FF00FF filling every non-spirit pixel. NO checkerboard. NO text.
"""


def token() -> str:
    return json.loads(AUTH.read_text())["credential_pool"]["xai-oauth"][0][
        "access_token"
    ]


def data_uri(path: Path) -> str:
    return f"data:image/png;base64,{base64.b64encode(path.read_bytes()).decode()}"


def edit(name: str, prompt: str, aspect: str) -> Path | None:
    dest = OUT / f"{name}.png"
    body = {
        "model": "grok-imagine-image-2.0",
        "prompt": prompt,
        "n": 1,
        "aspect_ratio": aspect,
        "resolution": "1k",
        "response_format": "b64_json",
        "image": {"url": data_uri(SRC), "type": "image_url"},
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
    edit("spirit_portrait_v3", PORTRAIT, "2:3")
    edit("spirit_idle_strip_v2", STRIP, "16:9")
