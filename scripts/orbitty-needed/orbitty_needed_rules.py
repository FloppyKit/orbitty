#!/usr/bin/env python3
"""orbitty_needed_rules — Needed standing rules: store + matcher.

GO-ORBITTY-NEEDED-RULES-STORE (Track B, scripts only).
Lock: ~/.buzz/PLANS/ORBITTY_NEEDED_RULES_LOCK_v0.md

Rules are ALWAYS user-minted (Matt). Nest/Spark/clerks never invent one;
they may only run `orbitty-needed rule from-answer ID` when Matt's answer
already contains a RULE: line. Match is a small AND param list, no LLM.

STORE stage: `open` classifies + logs hit/miss/collide and prints
RULE_HIT on stderr, but never auto-answers. Hiding a card waits
GO-ORBITTY-NEEDED-RULES-OPEN (habitat).
"""
from __future__ import annotations

import json
import os
import random
import re
import time
from pathlib import Path
from typing import Any

HOME = Path.home()
BUS = HOME / ".buzz" / "ops" / "needed-bus"
# GO-ORBITTY-GO-STANDARD: GOs and plans come from plansDir, the shared
# setting the app reads (~/.config/orbitty/settings.json `plansDir`,
# default ~/Orbitty GOs). ~/.buzz/PLANS is a read-only fallback; this
# module never writes a plan.
SETTINGS_PATH = HOME / ".config" / "orbitty" / "settings.json"
BUZZ_PLANS = HOME / ".buzz" / "PLANS"
RULES_PATH = BUS / "rules.json"
RULES_LOG = BUS / "rules.log"
ISSUED_PATH = BUS / "issued.json"

VERSION = 1
CAP_ENABLED = 48

GO_RE = re.compile(r"\b(GO-[A-Z0-9][-A-Z0-9]*)\b")
RULE_LINE_RE = re.compile(r"^RULE:\s*(.*)$")

# Small param list — only these (lock). Unknown keys on write = error.
PARAM_KEYS = ("flavor", "go", "prompt_kind", "project", "title_prefix", "title_exact")
WHEN_KEYS = ("issued",)
ISSUED_VALUES = ("shipped", "running", "absent")
FLAVORS = ("dogfood", "writer", "writer-leftover", "fact")
PROMPT_KINDS = ("first", "leftover")

_B36 = "0123456789abcdefghijklmnopqrstuvwxyz"


class RuleError(Exception):
    """Fail-closed write error. Message is shown to the caller; no write happened."""


def _b36(n: int) -> str:
    if n <= 0:
        return "0"
    out = []
    while n:
        n, r = divmod(n, 36)
        out.append(_B36[r])
    return "".join(reversed(out))


def new_rule_id() -> str:
    """r-<base36 millis>-<6 base36 rand>"""
    rand = random.SystemRandom().randrange(36**6)
    return f"r-{_b36(int(time.time() * 1000))}-{_b36(rand).rjust(6, '0')}"


def _utc_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


# ---------- store ----------


def load_store() -> dict:
    """version 1, rules: [] if missing. Raises RuleError if present but unreadable."""
    if not RULES_PATH.is_file():
        return {"version": VERSION, "updated_at": _utc_now(), "rules": []}
    try:
        d = json.loads(RULES_PATH.read_text())
    except (OSError, json.JSONDecodeError) as e:
        raise RuleError(f"rules store unreadable: {e}") from e
    if not isinstance(d, dict) or not isinstance(d.get("rules"), list):
        raise RuleError("rules store malformed: want {version, rules: []}")
    d.setdefault("version", VERSION)
    return d


def save_store(d: dict) -> None:
    """Atomic: tmp + replace."""
    BUS.mkdir(parents=True, exist_ok=True)
    d["version"] = VERSION
    d["updated_at"] = _utc_now()
    tmp = RULES_PATH.with_suffix(".json.tmp")
    tmp.write_text(json.dumps(d, indent=2) + "\n")
    tmp.replace(RULES_PATH)


def log_event(ev: dict) -> None:
    """Append-only JSONL. No PHI — titles only."""
    BUS.mkdir(parents=True, exist_ok=True)
    rec = {"ts": _utc_now()}
    rec.update(ev)
    try:
        with RULES_LOG.open("a") as f:
            f.write(json.dumps(rec) + "\n")
    except OSError:
        pass


# ---------- classify (no model) ----------


def extract_go(text: str) -> str | None:
    if not text:
        return None
    m = GO_RE.search(text.upper().replace("?", " "))
    return m.group(1) if m else None


def why_value(why: str, key: str) -> str:
    """First 'key=value' token in a doing-fire why line ("" when absent).
    The fact title `Who runs this doing GO?` carries no GO token, so the
    why (`card=… go=GO-… planet=…`) is where `go` matches."""
    for tok in (why or "").split():
        if tok.startswith(key + "="):
            v = tok.split("=", 1)[1].strip()
            if v:
                return v
    return ""


def classify_title(title: str, project: str = "", why: str = "") -> dict:
    """Title → flavor / go / prompt_kind, per the lock table. `go` falls
    back to the why's `go=` when the title has no token (doing-fire)."""
    t = (title or "").strip()
    low = t.lower()
    flavor = "fact"
    prompt_kind: str | None = None
    if t.startswith("Dogfood ") and extract_go(t):
        flavor = "dogfood"
        if "leftover" in low:
            prompt_kind = "leftover"
    elif t.startswith("Writer for ") and t.endswith("?"):
        if "leftover" in low:
            flavor = "writer-leftover"
            prompt_kind = "leftover"
        else:
            flavor = "writer"
            prompt_kind = "first"
    return {
        "title": t,
        "flavor": flavor,
        "go": extract_go(t) or (why_value(why, "go").upper() or None),
        "prompt_kind": prompt_kind,
        "project": project or "",
    }


# ---------- RULE: parse ----------


def parse_kv_tokens(text: str) -> tuple[dict, dict]:
    """'flavor=dogfood issued=shipped' → (params, when). issued= → when.
    `title_exact` / `title_prefix` values may contain spaces: either key
    slurps the REST of the line, so it is always the last token
    (`RULE: flavor=fact title_exact=Who runs this doing GO?`).
    Unknown keys = error (fail closed, no write)."""
    params: dict[str, str] = {}
    when: dict[str, str] = {}
    rest = (text or "").strip()
    toks: list[str] = []
    while rest:
        low = rest.lower()
        if low.startswith("title_exact=") or low.startswith("title_prefix="):
            toks.append(rest)
            break
        parts = rest.split(None, 1)
        toks.append(parts[0])
        rest = parts[1].strip() if len(parts) > 1 else ""
    for tok in toks:
        if "=" not in tok:
            raise RuleError(f"bad RULE token {tok!r} (want k=v)")
        k, v = tok.split("=", 1)
        k = k.strip().lower()
        v = v.strip()
        if not v:
            raise RuleError(f"empty value for {k!r}")
        if k == "go":
            v = v.upper()
            # A trailing `*` is a prefix match (GO-ORBITTY*). A star anywhere
            # else — or a bare `*` wildcard — is not a param; refuse it.
            if "*" in v and (not v.endswith("*") or v.count("*") != 1 or len(v) == 1):
                raise RuleError(f"go={v!r}: '*' only as a trailing prefix mark (GO-ORBITTY*)")
        if k in WHEN_KEYS:
            if v not in ISSUED_VALUES:
                raise RuleError(f"issued={v!r} not one of {ISSUED_VALUES}")
            when[k] = v
        elif k in PARAM_KEYS:
            if k == "flavor" and v not in FLAVORS:
                raise RuleError(f"flavor={v!r} not one of {FLAVORS}")
            if k == "prompt_kind" and v not in PROMPT_KINDS:
                raise RuleError(f"prompt_kind={v!r} not one of {PROMPT_KINDS}")
            params[k] = v
        else:
            raise RuleError(
                f"unknown rule key {k!r} (params: {', '.join(PARAM_KEYS)}; when: issued)"
            )
    return params, when


def parse_rule_lines(answer: str) -> tuple[str, dict, dict, bool]:
    """Trailing ^RULE: lines → (stripped_answer, params, when, found).

    Stripped answer is what seat/green/leftover parsers should see; the
    RULE lines are teaching, not the dispatch text."""
    lines = (answer or "").splitlines()
    found = False
    params: dict[str, str] = {}
    when: dict[str, str] = {}
    i = len(lines)
    while i > 0:
        m = RULE_LINE_RE.match(lines[i - 1].strip())
        if not m:
            break
        found = True
        p, w = parse_kv_tokens(m.group(1))
        params.update(p)
        when.update(w)
        i -= 1
    stripped = "\n".join(lines[:i]).strip()
    return stripped, params, when, found


def strip_rule_lines(answer: str) -> str:
    stripped, _p, _w, _f = parse_rule_lines(answer)
    return stripped


# ---------- when.issued ----------


def plans_dir() -> Path:
    try:
        data = json.loads(SETTINGS_PATH.read_text())
    except (OSError, ValueError):
        data = {}
    raw = data.get("plansDir") if isinstance(data, dict) else None
    if isinstance(raw, str) and raw.strip():
        return Path(raw.strip()).expanduser()
    return HOME / "Orbitty GOs"


def plans_read_dirs() -> list[Path]:
    """plansDir first, then ~/.buzz/PLANS (read-only back-compat)."""
    primary = plans_dir()
    out = [primary]
    if BUZZ_PLANS.is_dir() and BUZZ_PLANS != primary:
        out.append(BUZZ_PLANS)
    return out


def leftover_prompt_for(go: str) -> Path | None:
    """Newest leftover papers file for this GO.

    Packets and `_GO_*_LEFTOVER*_PROMPT.md` compete by mtime. Leftover
    is papers only — a newer leftover file is not a new writer issue.
    """
    cands: list[Path] = []
    for pkt in BUS.glob(f"launch-{go}-leftover*.json"):
        try:
            raw = json.loads(pkt.read_text()).get("prompt") or ""
        except (OSError, json.JSONDecodeError):
            raw = ""
        p = Path(raw).expanduser() if raw else Path()
        if p.is_file():
            cands.append(p)
    slug = "_" + go.replace("-", "_") + "_LEFTOVER"
    for d in plans_read_dirs():
        cands.extend(p for p in d.glob(f"{slug}*_PROMPT.md") if p.is_file())
    if not cands:
        return None
    return max(cands, key=lambda p: p.stat().st_mtime)


def first_prompt_for(go: str) -> Path | None:
    pkt = BUS / f"launch-{go}.json"
    if pkt.is_file():
        try:
            raw = json.loads(pkt.read_text()).get("prompt") or ""
        except (OSError, json.JSONDecodeError):
            raw = ""
        p = Path(raw).expanduser() if raw else Path()
        if p.is_file():
            return p
    slug = f"_{go.replace('-', '_')}_PROMPT.md"
    for d in plans_read_dirs():
        p = d / slug
        if p.is_file():
            return p
    return None


def issued_state(go: str, prompt: str | None = None) -> str:
    """shipped | running | absent for this GO (any prompt).

    One launch per GO. A leftover papers file is not a new issue — if
    this GO already ran or shipped, `when.issued` still sees that.
    `prompt` is ignored for the cap (kept in the signature for callers).
    """
    del prompt  # one launch per GO — leftover path is not a new issue
    if not go or not ISSUED_PATH.is_file():
        return "absent"
    try:
        d = json.loads(ISSUED_PATH.read_text())
    except (OSError, json.JSONDecodeError):
        return "absent"
    rec = d.get(go.upper()) if isinstance(d, dict) else None
    if not isinstance(rec, dict):
        return "absent"
    if rec.get("shipped"):
        return "shipped"
    pid = int(rec.get("pid") or 0)
    if pid > 0:
        try:
            os.kill(pid, 0)
            return "running"
        except OSError:
            pass
    return "absent"


# ---------- match ----------


def go_param_matches(rule_value: str, card_go: str) -> bool:
    """A trailing `*` is a prefix match (GO-ORBITTY*); without the star the
    match stays exact. Case-insensitive either way."""
    rv = (rule_value or "").upper()
    cv = (card_go or "").upper()
    if not cv:
        return False
    if rv.endswith("*"):
        prefix = rv[:-1]
        return bool(prefix) and cv.startswith(prefix)
    return cv == rv


def rule_go(rule: dict, card: dict) -> str:
    """The GO a rule acts on for when.issued / prompt lookups: the param,
    unless it is a prefix pattern — GO-ORBITTY* itself names no prompt
    file, so the card's concrete token is the lookup key."""
    pg = (rule.get("params") or {}).get("go") or ""
    if pg and not pg.endswith("*"):
        return pg
    return card.get("go") or ""


def rule_matches(rule: dict, card: dict) -> bool:
    """AND: every specified param must match; omitted = wildcard."""
    params = rule.get("params") or {}
    for k, v in params.items():
        if k == "flavor" and card.get("flavor") != v:
            return False
        if k == "go" and not go_param_matches(v, card.get("go") or ""):
            return False
        if k == "prompt_kind" and card.get("prompt_kind") != v:
            return False
        if k == "project" and (card.get("project") or "") != v:
            return False
        if k == "title_prefix" and not (card.get("title") or "").startswith(v):
            return False
        if k == "title_exact" and (card.get("title") or "") != v:
            return False
    when = rule.get("when") or {}
    if "issued" in when:
        go = rule_go(rule, card)
        prompt = card.get("prompt") or ""
        if not prompt and go:
            kind = params.get("prompt_kind") or card.get("prompt_kind")
            p = leftover_prompt_for(go) if kind == "leftover" else first_prompt_for(go)
            prompt = str(p) if p else ""
        if issued_state(go, prompt or None) != when["issued"]:
            return False
    return True


def match_rules(store: dict, card: dict) -> list[dict]:
    return [
        r
        for r in store.get("rules") or []
        if r.get("enabled", True) and rule_matches(r, card)
    ]


def is_collision(hits: list[dict]) -> bool:
    """2+ hits with different answer strings. Same answer = one logical hit."""
    answers = {(h.get("answer") or "").strip() for h in hits}
    return len(answers) > 1


# ---------- write ----------


def validate(params: dict, when: dict, answer: str) -> None:
    for k in params:
        if k not in PARAM_KEYS:
            raise RuleError(f"unknown param key {k!r}")
    for k in when:
        if k not in WHEN_KEYS:
            raise RuleError(f"unknown when key {k!r}")
    if "issued" in when and not params.get("go"):
        raise RuleError("when.issued requires params.go — refusing to save")
    if not (answer or "").strip():
        raise RuleError("standing answer is empty — a rule with no answer can never run")


def add_rule(
    params: dict,
    when: dict,
    answer: str,
    *,
    source_needed_id: str | None = None,
    note: str | None = None,
) -> dict:
    store = load_store()
    enabled = [r for r in store["rules"] if r.get("enabled", True)]
    if len(enabled) >= CAP_ENABLED:
        raise RuleError(f"cap {CAP_ENABLED} enabled rules — fail closed, ping Matt")
    validate(params, when, answer)
    rule: dict[str, Any] = {
        "id": new_rule_id(),
        "enabled": True,
        "created_at": _utc_now(),
        "source_needed_id": source_needed_id,
        "params": params,
        "answer": answer.strip(),
    }
    if when:
        rule["when"] = when
    if note:
        rule["note"] = note
    store["rules"].append(rule)
    save_store(store)
    log_event({"event": "mint", "rule": rule["id"], "params": params, "when": when,
               "source_needed_id": source_needed_id})
    return rule


def remove_rule(rid: str) -> dict:
    store = load_store()
    rules = store["rules"]
    for i, r in enumerate(rules):
        if r.get("id") == rid:
            dead = rules.pop(i)
            save_store(store)
            log_event({"event": "rm", "rule": rid})
            return dead
    raise RuleError(f"no rule {rid}")
