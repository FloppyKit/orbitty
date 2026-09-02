"""Frozen DQ8×SoA law — never invent past AXISLOCK.md."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VROID_BRAND = ROOT.parent
AXISLOCK_PATH = VROID_BRAND / "AXISLOCK.md"


class AxisLockError(RuntimeError):
    pass


REQUIRED_PHRASES = [
    "1.5–2×",
    "Toriyama",
    "cel outline",
    "travelogue",
    "PS2",
    "Forbidden",
]


def load_axislock(path: Path | None = None) -> str:
    p = path or AXISLOCK_PATH
    if not p.is_file():
        raise AxisLockError(f"AXISLOCK missing: {p}")
    text = p.read_text(encoding="utf-8")
    missing = [s for s in REQUIRED_PHRASES if s not in text]
    if missing:
        raise AxisLockError(f"AXISLOCK incomplete; missing {missing}")
    return text


def assert_axislock() -> Path:
    load_axislock()
    return AXISLOCK_PATH
