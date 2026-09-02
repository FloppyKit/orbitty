"""Real HID clicks for Unity (VRoid). macos-harness PID clicks are ignored."""
from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path

CALIB = Path(__file__).resolve().parents[1] / "calibrations" / "default.json"


def load_meta() -> dict:
    if CALIB.is_file():
        return json.loads(CALIB.read_text()).get("meta") or {}
    return {
        "bounds": {"x": 704.0, "y": 50.0, "width": 1077.0, "height": 956.0},
        "scale_x": 1.1884865366759516,
        "scale_y": 1.1882845188284519,
    }


def image_to_screen(ix: float, iy: float) -> tuple[int, int]:
    m = load_meta()
    b = m.get("bounds") or {}
    sx = float(m.get("scale_x") or 1.0)
    sy = float(m.get("scale_y") or 1.0)
    return int(round(float(b.get("x", 0)) + ix / sx)), int(
        round(float(b.get("y", 0)) + iy / sy)
    )


def activate(app: str = "VRoid Studio") -> None:
    subprocess.run(
        ["osascript", "-e", f'tell application "{app}" to activate'], check=False
    )
    time.sleep(0.45)


def click_image(ix: float, iy: float, *, pause: float = 0.35) -> tuple[int, int]:
    x, y = image_to_screen(ix, iy)
    activate()
    subprocess.run(["cliclick", f"c:{x},{y}"], check=False)
    time.sleep(pause)
    return x, y


def drag_image(
    ix1: float, iy1: float, ix2: float, iy2: float, *, steps: int = 25
) -> None:
    activate()
    x1, y1 = image_to_screen(ix1, iy1)
    x2, y2 = image_to_screen(ix2, iy2)
    subprocess.run(["cliclick", f"m:{x1},{y1}"], check=False)
    time.sleep(0.05)
    subprocess.run(["cliclick", f"dd:{x1},{y1}"], check=False)
    time.sleep(0.05)
    for i in range(1, steps + 1):
        t = i / steps
        x = int(x1 + (x2 - x1) * t)
        y = int(y1 + (y2 - y1) * t)
        subprocess.run(["cliclick", f"m:{x},{y}"], check=False)
        time.sleep(0.012)
    subprocess.run(["cliclick", f"du:{x2},{y2}"], check=False)
    time.sleep(0.35)
