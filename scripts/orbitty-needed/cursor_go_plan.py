#!/usr/bin/env python3
"""cursor_go_plan — what cursor-go hands cursor-agent, from the GO itself.

GO-ORBITTY-GO-STANDARD follow-up. cursor-go calls this; nothing here
writes anything.

- `prompt GO`: the GO's `_GO_X_PROMPT.md`, looked up in plansDir first,
  then the read-only ~/.buzz/PLANS copy. plansDir is the shared setting
  the app reads (~/.config/orbitty/settings.json `plansDir`), default
  ~/Orbitty GOs.
- `args PROMPT`: one argument per line for cursor-agent, or exit 5 with a
  reason on stderr (fail closed).

Model: the GO header `model`, else the defaultRunner setting's model when
that runner is a Cursor one (`defaultRunner.harness` cursor or blank),
else grok-4.7-high. The model must pass the Rust launch_model rule
(agents::validate_token): no leading '-', 1-100 characters from letters,
digits and . _ : / @ + -. It travels as ONE argument, `--model=<m>`.

--force (and --trust, which the app's cursor door pairs with it) only for
the full-auto preset, same as agents::cursor_agent_args:
- a header `permissions:` value wins: read-only | workspace-write |
  full-auto (anything else reads as read-only, like
  agents::normalize_permissions);
- otherwise the header `tools` decide: any of shell, git_commit, git_push,
  install_nightly, orbittyctl → full-auto; edit alone → workspace-write;
  read only, no tools, or no header → read-only (no --force).
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HOME = Path.home()
SETTINGS_PATH = HOME / ".config" / "orbitty" / "settings.json"
# Read-only back-compat. Never a write target.
BUZZ_PLANS = HOME / ".buzz" / "PLANS"
LAST_FALLBACK_MODEL = "grok-4.7-high"
MODEL_RE = re.compile(r"^[A-Za-z0-9._:/@+-]{1,100}$")
FULL_AUTO_TOOLS = frozenset({"shell", "git_commit", "git_push", "install_nightly", "orbittyctl"})
WRITE_TOOLS = frozenset({"edit"})


class PlanError(Exception):
    pass


def settings() -> dict:
    try:
        data = json.loads(SETTINGS_PATH.read_text())
    except (OSError, ValueError):
        return {}
    return data if isinstance(data, dict) else {}


def plans_dir() -> Path:
    raw = settings().get("plansDir")
    if isinstance(raw, str) and raw.strip():
        return Path(raw.strip()).expanduser()
    return HOME / "Orbitty GOs"


def plans_read_dirs() -> list[Path]:
    primary = plans_dir()
    out = [primary]
    if BUZZ_PLANS.is_dir() and BUZZ_PLANS != primary:
        out.append(BUZZ_PLANS)
    return out


def prompt_for(go: str) -> Path:
    slug = f"_{go.replace('-', '_')}_PROMPT.md"
    for d in plans_read_dirs():
        p = d / slug
        if p.is_file():
            return p
    return plans_dir() / slug


def _scalar(v: str) -> str:
    v = v.strip()
    if v[:1] in ("'", '"'):
        end = v.find(v[0], 1)
        return (v[1:end] if end > 0 else v[1:]).strip()
    if v.startswith("#"):
        return ""
    return re.split(r"\s+#", v, maxsplit=1)[0].strip()


def _list(v: str) -> list[str]:
    v = _scalar(v)
    if v.startswith("[") and v.endswith("]"):
        return [_scalar(x) for x in v[1:-1].split(",") if _scalar(x)]
    return [v] if v else []


def header(text: str) -> dict:
    """Top-level scalars and lists of the GO's YAML front matter. A packet
    starts with a `Workspace:` line; that and blank lines are skipped."""
    lines = text.splitlines()
    i = 0
    while i < len(lines) and (not lines[i].strip() or lines[i].startswith("Workspace:")):
        i += 1
    if i >= len(lines) or lines[i].strip() != "---":
        return {}
    out: dict = {}
    last = None
    for line in lines[i + 1:]:
        if line.strip() == "---":
            return out
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line[0] in " \t":
            item = line.strip()
            if last and item.startswith("- "):
                cur = out.get(last)
                if not isinstance(cur, list):
                    cur = []
                cur.append(_scalar(item[2:]))
                out[last] = cur
            continue
        if ":" not in line:
            last = None
            continue
        key, val = line.split(":", 1)
        key = key.strip()
        last = key
        out[key] = _list(val) if key == "tools" else _scalar(val)
    return {}  # no closing fence: not a header


def validate_model(model: str) -> None:
    if model.startswith("-"):
        raise PlanError(f"invalid model {model!r}: a model may not start with '-' (it would read as a flag)")
    if not MODEL_RE.match(model):
        raise PlanError(
            f"invalid model {model!r}: use 1-100 characters from letters, digits and . _ : / @ + -"
        )


def default_runner_model() -> str:
    dr = settings().get("defaultRunner")
    if not isinstance(dr, dict):
        return ""
    harness = str(dr.get("harness") or "cursor").strip().lower()
    model = str(dr.get("model") or "").strip()
    return model if harness == "cursor" else ""


def resolve_model(h: dict) -> tuple[str, str]:
    m = h.get("model")
    if isinstance(m, str) and m.strip():
        return m.strip(), "header"
    m = default_runner_model()
    if m:
        return m, "defaultRunner"
    return LAST_FALLBACK_MODEL, "fallback"


def normalize_permissions(raw: str) -> str:
    v = (raw or "").strip().lower()
    if v in ("workspace-write", "workspace_write"):
        return "workspace-write"
    if v in ("full-auto", "full_auto", "fullauto"):
        return "full-auto"
    return "read-only"


def preset(h: dict) -> str:
    perm = h.get("permissions")
    if isinstance(perm, str) and perm.strip():
        return normalize_permissions(perm)
    tools = h.get("tools")
    tools = {str(t).strip().lower() for t in tools} if isinstance(tools, list) else set()
    if tools & FULL_AUTO_TOOLS:
        return "full-auto"
    if tools & WRITE_TOOLS:
        return "workspace-write"
    return "read-only"


def plan(text: str) -> dict:
    h = header(text)
    model, source = resolve_model(h)
    validate_model(model)
    p = preset(h)
    args = [f"--model={model}"]
    if p == "full-auto":
        args += ["--force", "--trust"]
    return {"model": model, "source": source, "preset": p, "args": args}


def main(argv: list[str]) -> int:
    if len(argv) == 2 and argv[0] == "prompt":
        print(prompt_for(argv[1]))
        return 0
    if len(argv) == 2 and argv[0] == "args":
        try:
            text = Path(argv[1]).read_text(errors="replace")
            p = plan(text)
        except (OSError, PlanError) as e:
            print(f"cursor-go: {e}; not launching.", file=sys.stderr)
            return 5
        print("\n".join(p["args"]))
        return 0
    print("usage: cursor_go_plan.py prompt GO | args PROMPT", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
