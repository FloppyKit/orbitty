#!/usr/bin/env python3
"""orbitty_leftover_clerk — Dogfood leftover bounce → papers only.

GO-ORBITTY-LEFTOVER-CLERK (Track B, scripts only).
Lock: ~/.buzz/PLANS/ORBITTY_WRITER_DONE_NEEDED_LOCK_v0.md

GO-ORBITTY-GO-STANDARD: the caller passes the configured plans_dir. The
leftover papers are written there, and the first-cut prompt is read there
first. ~/.buzz/PLANS is a read-only fallback: nothing is ever written or
created under ~/.buzz.

A leftover bounce writes `_GO_*_LEFTOVER_*_PROMPT.md` (GO, HEAD, bounce
quote). It does **not** mint a Writer leftover card, does **not** write
a launch packet, and does **not** call cursor-go. One Cursor launch per
GO — leftover is the bounce record, not a second issue. If the template
cannot fill (no GO, no first-cut prompt path) the caller falls through
to the clerk ping — never invent scope the bounce did not name.
"""
from __future__ import annotations

import re
import subprocess
import time
from pathlib import Path

HOME = Path.home()
# Read-only back-compat. Never a write target.
PLANS = HOME / ".buzz" / "PLANS"
WS_DEFAULT = HOME / "orbitty"
# orbitty-needed checks this before calling handle(); an older clerk
# without it is never called (it would write into ~/.buzz/PLANS).
PLANS_DIR_AWARE = True


def default_plans_dir() -> Path:
    return HOME / "Orbitty GOs"


def under_buzz(path: Path) -> bool:
    buzz = (HOME / ".buzz").resolve()
    try:
        path.expanduser().resolve().relative_to(buzz)
        return True
    except ValueError:
        return False

GO_RE = re.compile(r"\b(GO-[A-Z0-9][-A-Z0-9]*)\b")
LEAD_STRIP_RE = re.compile(r"^(?:leftover|bounce)\b\s*[:\-]?\s*", re.I)
WORD_RE = re.compile(r"[A-Za-z0-9]+")

# Frozen. Papers only — not a writer launch prompt.
TEMPLATE = """# Leftover papers — {go}

Not a writer launch. One Cursor launch per GO. This file is the bounce record.

Workspace: {workspace}
HEAD: {head_short} {head_subject}
HEAD commit: {head_full}

Matt {date} Dogfood `{cid}` on `{head_short}`. Did **not** stamp. Bounce:

{quote}

## Named leftover

{lead}

## Dogfood (from first-cut, for eyes-on)

{dogfood}
"""


def extract_go(text: str) -> str | None:
    if not text:
        return None
    m = GO_RE.search(text.upper().replace("?", " "))
    return m.group(1) if m else None


def git(ws: Path, *args: str) -> str:
    try:
        return subprocess.check_output(
            ["git", "-C", str(ws)] + list(args),
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return ""


def workspace_from_prompt(path: Path) -> Path:
    if path.is_file():
        for line in path.read_text().splitlines():
            if line.startswith("Workspace:"):
                ws = Path(line.split(":", 1)[1].strip()).expanduser()
                if str(ws):
                    return ws
    return WS_DEFAULT


def prompt_dogfood_section(path: Path) -> str:
    """The first-cut prompt's `## Dogfood` block (fallback when the card
    carried no why)."""
    if not path.is_file():
        return ""
    lines: list[str] = []
    in_dog = False
    for line in path.read_text().splitlines():
        if line.startswith("## Dogfood"):
            in_dog = True
            continue
        if in_dog and line.startswith("## "):
            break
        if in_dog:
            lines.append(line)
    return "\n".join(lines).strip()


def slug_from_answer(answer: str) -> tuple[str, str]:
    """(FILENAME_SLUG, "commit words") from the bounce's first words."""
    body = LEAD_STRIP_RE.sub("", (answer or "").strip())
    words = WORD_RE.findall(body)[:4]
    if not words:
        return "BOUNCE", "bounce"
    return "_".join(w.upper() for w in words), " ".join(w.lower() for w in words)


def lead_from_answer(answer: str) -> str:
    """First line, first 8 words — the title's verbatim topic."""
    first = (answer or "").strip().splitlines()[0] if answer.strip() else ""
    first = LEAD_STRIP_RE.sub("", first).strip()
    words = first.split()
    lead = " ".join(words[:8])
    if len(words) > 8:
        lead += " …"
    return lead or "bounce"


def quote_lines(answer: str) -> str:
    return "\n".join("> " + line for line in (answer or "").strip().splitlines())


def unique_prompt_path(go: str, slug: str, plans: Path) -> Path:
    stem = f"_{go.replace('-', '_')}_LEFTOVER_{slug}"
    p = plans / f"{stem}_PROMPT.md"
    if not p.exists():
        return p
    for n in range(2, 20):
        p = plans / f"{stem}_{n}_PROMPT.md"
        if not p.exists():
            return p
    return plans / f"{stem}_{int(time.time())}_PROMPT.md"


def first_prompt_path(go: str, plans: Path, first_prompt: str | None = None) -> Path | None:
    """The GO's first-cut prompt: the file the caller resolved (the sealed
    GO), else `_GO_X_PROMPT.md` in plans_dir, else the read-only
    ~/.buzz/PLANS copy."""
    if first_prompt:
        p = Path(first_prompt).expanduser()
        if p.is_file():
            return p
    slug = f"_{go.replace('-', '_')}_PROMPT.md"
    for d in (plans, PLANS):
        p = d / slug
        if p.is_file():
            return p
    return None


def render(slots: dict[str, str]) -> str:
    out = TEMPLATE
    for k, v in slots.items():
        out = out.replace("{" + k + "}", v)
    return out


def handle(
    rec: dict,
    *,
    script_path: str = "",
    plans_dir: str | Path | None = None,
    first_prompt: str | None = None,
    workspace: str | None = None,
) -> dict:
    """Dogfood leftover bounce → leftover papers file in plans_dir. No
    Writer card. ok=False means the template cannot fill — clerk ping."""
    answer = (rec.get("neededAnswer") or "").strip()
    title = (rec.get("title") or "").strip()
    cid = rec.get("id") or ""
    go = extract_go(title)
    if not go:
        return {"ok": False, "reason": "no GO in title"}
    if not answer:
        return {"ok": False, "reason": "empty answer"}
    plans = Path(plans_dir).expanduser() if plans_dir else default_plans_dir()
    if under_buzz(plans):
        return {"ok": False, "reason": f"plans_dir {plans} is under ~/.buzz; not writing there"}
    first = first_prompt_path(go, plans, first_prompt)
    if first is None:
        return {"ok": False, "reason": f"no first-cut prompt _{go.replace('-', '_')}_PROMPT.md"}
    ws = Path(workspace).expanduser() if workspace else workspace_from_prompt(first)
    head_full = git(ws, "rev-parse", "HEAD")
    if not head_full:
        return {"ok": False, "reason": f"no HEAD in {ws}"}
    head_subject = git(ws, "log", "-1", "--pretty=%s")
    head_short = head_full[:7]
    dogfood = (rec.get("neededWhy") or rec.get("needed_why") or "").strip()
    if not dogfood:
        dogfood = prompt_dogfood_section(first)
    if not dogfood:
        return {"ok": False, "reason": "no dogfood steps on card or first-cut prompt"}
    slug, _slug_words = slug_from_answer(answer)
    path = unique_prompt_path(go, slug, plans)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        render(
            {
                "go": go,
                "lead": lead_from_answer(answer),
                "workspace": str(ws),
                "head_short": head_short,
                "head_subject": head_subject,
                "head_full": head_full,
                "date": time.strftime("%Y-%m-%d"),
                "cid": cid,
                "quote": quote_lines(answer),
                "dogfood": dogfood,
            }
        )
    )
    return {
        "ok": True,
        "go": go,
        "prompt": str(path),
        "packet": "",
        "card_id": None,
        "card_minted": False,
        "slug": slug,
    }
