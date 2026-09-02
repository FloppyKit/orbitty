"""VRoid Studio driver over macos-harness."""
from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

APP = "VRoid Studio"
ROOT = Path(__file__).resolve().parents[1]
CAPTURES = ROOT / "captures"
CALIBRATIONS = ROOT / "calibrations"
MODELS = ROOT.parent / "models"


@dataclass
class SeeResult:
    path: Path | None
    meta: dict[str, Any]


class VRoidDriver:
    def __init__(self, mac: Any | None = None):
        self.mac = mac
        CAPTURES.mkdir(parents=True, exist_ok=True)
        CALIBRATIONS.mkdir(parents=True, exist_ok=True)
        MODELS.mkdir(parents=True, exist_ok=True)

    def _run_mh(self, code: str) -> str:
        proc = subprocess.run(
            ["macos-harness"],
            input=code,
            text=True,
            capture_output=True,
        )
        if proc.returncode != 0:
            raise RuntimeError(
                f"macos-harness failed ({proc.returncode}): "
                f"{proc.stderr or proc.stdout}"
            )
        return proc.stdout

    def doctor(self) -> dict[str, Any]:
        if self.mac is not None:
            return self.mac.doctor()
        out = subprocess.run(
            ["macos-harness", "doctor"], capture_output=True, text=True
        )
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            return {"raw": out.stdout, "stderr": out.stderr, "code": out.returncode}

    def ensure_screen_recording(self) -> None:
        d = self.doctor()
        perms = d.get("permissions") or {}
        if perms.get("screen_recording") is not True:
            raise RuntimeError(
                "Screen Recording OFF for agent host (Berd.app). "
                "Enable it, Cmd+Q Berd, reopen, then retry."
            )
        if perms.get("accessibility") is not True:
            raise RuntimeError("Accessibility OFF for agent host.")

    def state(self) -> dict[str, Any]:
        if self.mac is not None:
            return self.mac.get_app_state(APP)
        code = f"import json\nprint(json.dumps(mac.get_app_state({APP!r}), default=str))\n"
        return json.loads(self._run_mh(code))

    def see(self, name: str | None = None) -> SeeResult:
        self.ensure_screen_recording()
        ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        path = CAPTURES / f"{ts}_{name or 'see'}.png"
        if self.mac is not None:
            meta = self.mac.see(APP, path=str(path))
            return SeeResult(path=path if path.exists() else None, meta=meta or {})
        code = f"""
import json
from pathlib import Path
p = Path({str(path)!r})
meta = mac.see({APP!r}, path=p)
print(json.dumps({{"path": str(p), "meta": meta}}, default=str))
"""
        data = json.loads(self._run_mh(code))
        p = Path(data.get("path") or path)
        return SeeResult(path=p if p.exists() else None, meta=data.get("meta") or {})

    def key(self, chord: str) -> None:
        if self.mac is not None:
            self.mac.key(chord, app=APP)
            return
        self._run_mh(f"mac.key({chord!r}, app={APP!r})\nprint('ok')\n")

    def type_text(self, text: str) -> None:
        if self.mac is not None:
            self.mac.type(text, app=APP)
            return
        self._run_mh(f"mac.type({text!r}, app={APP!r})\nprint('ok')\n")

    def click(self, x: float, y: float, *, clicks: int = 1) -> dict[str, Any]:
        if self.mac is not None:
            return self.mac.click(x, y, app=APP, clicks=clicks)
        code = (
            f"import json\n"
            f"print(json.dumps(mac.click({x}, {y}, app={APP!r}, clicks={clicks}), default=str))\n"
        )
        return json.loads(self._run_mh(code))

    def load_points(self, profile: str = "default") -> dict[str, Any]:
        p = CALIBRATIONS / f"{profile}.json"
        if not p.is_file():
            return {"profile": profile, "points": {}}
        return json.loads(p.read_text(encoding="utf-8"))

    def save_point(
        self,
        name: str,
        x: float,
        y: float,
        *,
        profile: str = "default",
        note: str = "",
    ) -> Path:
        data = self.load_points(profile)
        data.setdefault("points", {})[name] = {
            "x": x,
            "y": y,
            "note": note,
            "saved_at": datetime.now(timezone.utc).isoformat(),
        }
        data["profile"] = profile
        data["app"] = APP
        path = CALIBRATIONS / f"{profile}.json"
        path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
        return path

    def click_point(self, name: str, *, profile: str = "default") -> dict[str, Any]:
        pts = self.load_points(profile).get("points") or {}
        if name not in pts:
            raise KeyError(f"No point {name!r} in profile {profile!r}")
        pt = pts[name]
        return self.click(float(pt["x"]), float(pt["y"]))

    def raise_window(self) -> None:
        if self.mac is not None:
            try:
                self.mac.ax.perform(1, "AXRaise")
            except Exception:
                pass
            return
        self._run_mh(
            f"try:\n mac.ax.perform(1, 'AXRaise')\nexcept Exception as e:\n print(e)\nprint('ok')\n"
        )
