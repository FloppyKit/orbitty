#!/usr/bin/env python3
"""CLI for VRoid harness."""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from lib.axislock import assert_axislock, load_axislock, AxisLockError  # noqa: E402
from lib.driver import VRoidDriver  # noqa: E402


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="VRoid Studio harness (macos-harness)")
    sub = ap.add_subparsers(dest="cmd", required=True)

    sub.add_parser("doctor", help="Permissions + AxisLock")
    sub.add_parser("axislock", help="Print AxisLock card")
    sub.add_parser("state", help="AX state (no screenshot)")
    p_see = sub.add_parser("see", help="Screenshot (needs Screen Recording)")
    p_see.add_argument("--name", default="see")
    p_key = sub.add_parser("key", help="Key chord")
    p_key.add_argument("chord")
    p_click = sub.add_parser("click", help="Click screenshot coords")
    p_click.add_argument("x", type=float)
    p_click.add_argument("y", type=float)
    p_save = sub.add_parser("save-point", help="Save calibration point")
    p_save.add_argument("name")
    p_save.add_argument("x", type=float)
    p_save.add_argument("y", type=float)
    p_save.add_argument("--note", default="")
    p_save.add_argument("--profile", default="default")
    p_cp = sub.add_parser("click-point", help="Click saved point")
    p_cp.add_argument("name")
    p_cp.add_argument("--profile", default="default")
    sub.add_parser("list-points")
    sub.add_parser("recipes")

    args = ap.parse_args(argv)
    d = VRoidDriver(mac=None)

    if args.cmd == "doctor":
        try:
            path = assert_axislock()
            al = "ok"
        except AxisLockError as e:
            path = None
            al = str(e)
        doc = d.doctor()
        print(
            json.dumps(
                {
                    "axislock": al,
                    "axislock_path": str(path) if path else None,
                    "doctor": doc,
                },
                indent=2,
            )
        )
        perms = doc.get("permissions") or {}
        if perms.get("screen_recording") is not True:
            print(
                "\n→ Enable Screen Recording for Berd.app, Cmd+Q Berd, reopen.",
                file=sys.stderr,
            )
            return 2
        return 0 if al == "ok" else 1

    if args.cmd == "axislock":
        print(load_axislock())
        return 0

    if args.cmd == "state":
        print(json.dumps(d.state(), indent=2, default=str))
        return 0

    if args.cmd == "see":
        assert_axislock()
        r = d.see(args.name)
        print(
            json.dumps(
                {"path": str(r.path) if r.path else None},
                indent=2,
            )
        )
        if r.path:
            print(r.path)
        return 0

    if args.cmd == "key":
        d.key(args.chord)
        print("ok")
        return 0

    if args.cmd == "click":
        print(json.dumps(d.click(args.x, args.y), indent=2, default=str))
        return 0

    if args.cmd == "save-point":
        print(
            d.save_point(
                args.name, args.x, args.y, profile=args.profile, note=args.note
            )
        )
        return 0

    if args.cmd == "click-point":
        print(
            json.dumps(
                d.click_point(args.name, profile=args.profile),
                indent=2,
                default=str,
            )
        )
        return 0

    if args.cmd == "list-points":
        print(json.dumps(d.load_points(), indent=2))
        return 0

    if args.cmd == "recipes":
        for f in sorted((ROOT / "recipes").glob("*")):
            if f.name != "README.md":
                print(f.name)
        return 0

    return 1


if __name__ == "__main__":
    raise SystemExit(main())
