#!/usr/bin/env python3
"""THIN stamp writer. Dev instrument.

The app passes the stamp record on stdin and the output directory via
ORBITTY_THIN_OUTPUT or --output. This process only writes files. It does
not call back into the app, play a clip, or touch now.json.

  python3 vivo_thin.py stamp < payload.json
  python3 vivo_thin.py report --output DIR
  python3 vivo_thin.py dry < payload.json
"""
from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from chat_slam_lib import scan_assistant, scan_user, slam_align  # noqa: E402

REQUIRED = (
    "session_id",
    "turn_count",
    "context_chars",
    "context_tokens_est",
    "secs_since_reset",
    "last_reset_reason",
    "ts",
)


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def stamp_id() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")


class ThinUsage(Exception):
    pass


def output_dir(argv: list[str]) -> Path:
    if "--output" in argv:
        i = argv.index("--output")
        if i + 1 >= len(argv):
            raise ThinUsage("thin: --output needs a directory")
        return Path(argv[i + 1]).expanduser()
    import os

    env = os.environ.get("ORBITTY_THIN_OUTPUT", "").strip()
    if not env:
        raise ThinUsage(
            "thin: output dir is not configured (ORBITTY_THIN_OUTPUT or --output)"
        )
    return Path(env).expanduser()


def load_payload() -> dict[str, Any]:
    if sys.stdin.isatty():
        raise ThinUsage("thin: stamp reads a JSON payload on stdin")
    raw = sys.stdin.read()
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as e:
        raise ThinUsage(f"thin: payload is not JSON ({e})") from e
    if not isinstance(payload, dict):
        raise ThinUsage("thin: payload must be an object")
    missing = [k for k in REQUIRED if k not in payload]
    if missing:
        raise ThinUsage("thin: payload missing " + ", ".join(missing))
    return payload


def assign_turns(raw: list[Any], turn_count: int) -> list[dict[str, Any]]:
    cleaned: list[dict[str, Any]] = []
    for t in raw or []:
        if not isinstance(t, dict):
            continue
        role_in = str(t.get("role") or "").lower()
        if role_in in ("me", "user", "you", "human"):
            role = "user"
        else:
            role = "assistant"
        text = str(t.get("text") or t.get("line") or "")
        if not text.strip():
            continue
        cleaned.append({"role": role, "text": text})
    n = len(cleaned)
    start = (turn_count - n + 1) if n else turn_count
    if n and start < 1:
        start = 1
    out = []
    for i, t in enumerate(cleaned):
        t["turn"] = start + i
        out.append(t)
    return out


def scan_turns(turns: list[dict[str, Any]], turn_count: int) -> dict[str, Any]:
    prev = ""
    last_codes: list[str] = []
    recent: list[str] = []
    prev_err: str | None = None
    last_user: dict[str, Any] = {}
    pings: list[int] = []
    feats: list[dict[str, Any]] = []
    for t in turns:
        text = t.get("text") or ""
        if t.get("role") == "user":
            last_user = scan_user(text, prev_err)
            if last_user.get("error_sig"):
                prev_err = last_user["error_sig"]
            continue
        sc = scan_assistant(text, prev, recent[-9:], last_codes=last_codes)
        stuck = bool(last_user.get("task_stuck"))
        moved = bool(last_user.get("task_move"))
        ping = slam_align(
            bool(sc.get("nov_flat")), float(sc.get("consistency") or 0), stuck, moved
        )
        if not ping and sc.get("nov_flat") and (sc.get("slam_hits") or 0) >= 1 and not moved:
            ping = True
        row = {
            "turn": t.get("turn"),
            "ping": ping,
            "nov_flat": sc.get("nov_flat"),
            "slam_hits": sc.get("slam_hits"),
            "alliance_hits": sc.get("alliance_hits"),
            "task_stuck": stuck,
            "task_move": moved,
        }
        feats.append(row)
        if ping:
            pings.append(int(t.get("turn") or 0))
        prev = text
        if sc.get("this_code"):
            last_codes.append(sc["this_code"])
            last_codes = last_codes[-3:]
        recent.extend(sc.get("code_hashes") or [])
    first = pings[0] if pings else None
    human = turn_count
    return {
        "window_turns": len(turns),
        "n_turns": turn_count,
        "n_asst": sum(1 for t in turns if t.get("role") == "assistant"),
        "n_pings": len(pings),
        "first_ping_turn": first,
        "human_stamp_turn": human,
        "lag_turns": (human - first) if first is not None else None,
        "detector_before_human": first is not None,
        "human_before_detector": first is None,
        "pings": pings,
        "feats": feats,
    }


def write_stamp(pad: Path, payload: dict[str, Any]) -> Path:
    pad.mkdir(parents=True, exist_ok=True)
    turn_count = int(payload["turn_count"])
    turns = assign_turns(payload.get("turns") or [], turn_count)
    scan = scan_turns(turns, turn_count)
    sid = stamp_id()
    d = pad / sid
    n = 0
    while d.exists():
        n += 1
        d = pad / f"{sid}-{n}"
        sid = d.name
    d.mkdir(parents=True)
    (d / "turns.jsonl").write_text(
        "".join(json.dumps(t, ensure_ascii=False) + "\n" for t in turns)
    )
    event = {
        "id": sid,
        "ts": payload["ts"],
        "kind": "human_thin",
        "clinical_claim": False,
        "session_id": payload["session_id"],
        "turn_count": turn_count,
        "context_chars": payload["context_chars"],
        "context_tokens_est": payload["context_tokens_est"],
        "secs_since_reset": payload["secs_since_reset"],
        "last_reset_reason": payload["last_reset_reason"],
        "mode": payload.get("mode") or "",
        "written_at": utc_now(),
        **{k: v for k, v in scan.items() if k != "feats"},
    }
    (d / "scan.json").write_text(json.dumps({**scan, "event": event}, indent=2) + "\n")
    (d / "event.json").write_text(json.dumps(event, indent=2) + "\n")
    events = pad / "events.jsonl"
    with events.open("a") as f:
        f.write(json.dumps(event, ensure_ascii=False) + "\n")
    print(
        f"THIN {sid} session={event['session_id']} turns={turn_count} "
        f"context={event['context_chars']}ch/{event['context_tokens_est']}tok "
        f"since_reset={event['secs_since_reset']}s reason={event['last_reset_reason']}"
    )
    print(f"pad {d}")
    return d


def report(pad: Path) -> None:
    events = pad / "events.jsonl"
    rows = []
    if events.exists():
        for line in events.read_text().splitlines():
            if line.strip():
                rows.append(json.loads(line))
    print(f"n_stamps {len(rows)}")
    if not rows:
        print("no thin stamps yet")
        return
    for r in rows:
        print(
            f"{r.get('id', '')} session={r.get('session_id')} "
            f"turns={r.get('turn_count')} reason={r.get('last_reset_reason')}"
        )


def main(argv: list[str]) -> int:
    cmd = (argv[1] if len(argv) > 1 else "stamp").lower()
    try:
        if cmd in ("dry", "--dry"):
            payload = load_payload()
            turn_count = int(payload["turn_count"])
            turns = assign_turns(payload.get("turns") or [], turn_count)
            sc = scan_turns(turns, turn_count)
            sc.pop("feats", None)
            print(json.dumps(sc, indent=2))
            print("dry — no stamp written")
            return 0
        pad = output_dir(argv)
        if cmd == "report":
            report(pad)
            return 0
        if cmd in ("stamp", "thin"):
            write_stamp(pad, load_payload())
            return 0
    except ThinUsage as e:
        print(str(e), file=sys.stderr)
        return 2
    print("usage: vivo_thin.py stamp|report|dry", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
