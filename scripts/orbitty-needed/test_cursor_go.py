"""cursor-go + cursor_go_plan.py with a throwaway HOME.

The plan helper is unit-tested; cursor-go (zsh) runs for real with a stub
cursor-agent that records its argv, and a stub pgrep (no live writer).
Every test asserts nothing appears under $HOME/.buzz unless the test put
it there, and that what it put there is unchanged.
"""
from __future__ import annotations

import importlib.util
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
GO = "GO-ORBITTY-CG"
FULL = "---\nid: GO-ORBITTY-CG\nmodel: grok-4.7-high   # comment\ntools: [read, edit, shell, git_commit]\n---\nBODY\n"


def load_plan(home: Path):
    old = os.environ.get("HOME")
    os.environ["HOME"] = str(home)
    try:
        spec = importlib.util.spec_from_file_location("cursor_go_plan_t", str(HERE / "cursor_go_plan.py"))
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return mod
    finally:
        if old is None:
            os.environ.pop("HOME", None)
        else:
            os.environ["HOME"] = old


class HomeCase(unittest.TestCase):
    def setUp(self):
        self.home = Path(tempfile.mkdtemp(prefix="cursor-go-test-"))
        self.addCleanup(shutil.rmtree, self.home, True)
        self.buzz = self.home / ".buzz"
        self.plans = self.home / "Custom GOs"
        self.plans.mkdir()

    def settings(self, **kw):
        p = self.home / ".config" / "orbitty" / "settings.json"
        p.parent.mkdir(parents=True, exist_ok=True)
        data = {"plansDir": str(self.plans)}
        data.update(kw)
        p.write_text(json.dumps(data))

    def buzz_snapshot(self):
        if not self.buzz.exists():
            return None
        return sorted((str(p), p.read_bytes() if p.is_file() else b"") for p in self.buzz.rglob("*"))


class PlanRules(HomeCase):
    def plan(self, text):
        return load_plan(self.home).plan(text)

    def test_model_header_then_default_runner_then_fallback(self):
        self.settings(defaultRunner={"runner": "orbitty-cursor", "harness": "cursor", "model": "sonnet-4.5"})
        p = self.plan(FULL)
        self.assertEqual((p["model"], p["source"]), ("grok-4.7-high", "header"))
        self.assertEqual(p["args"][0], "--model=grok-4.7-high")
        no_model = FULL.replace("model: grok-4.7-high   # comment\n", "")
        p = self.plan(no_model)
        self.assertEqual((p["model"], p["source"]), ("sonnet-4.5", "defaultRunner"))
        self.settings()  # no defaultRunner
        p = self.plan(no_model)
        self.assertEqual((p["model"], p["source"]), ("grok-4.7-high", "fallback"))
        # A non-Cursor defaultRunner model is not a Cursor model.
        self.settings(defaultRunner={"harness": "herdr", "model": "grok"})
        self.assertEqual(self.plan(no_model)["source"], "fallback")

    def test_bad_model_refuses(self):
        mod = load_plan(self.home)
        for bad in ["--force", "-f", "--yolo", "m$(id)", "a b", "x;y", "m`id`", "x" * 101]:
            with self.assertRaises(mod.PlanError, msg=bad):
                mod.plan(FULL.replace("grok-4.7-high", bad if " " not in bad else f'"{bad}"'))
        self.settings(defaultRunner={"harness": "cursor", "model": "--force"})
        mod = load_plan(self.home)
        with self.assertRaises(mod.PlanError):
            mod.plan("no header\n")

    def test_force_only_for_full_auto(self):
        cases = {
            "tools: [read]": [],
            "tools: [read, edit]": [],
            "tools: [read, shell]": ["--force", "--trust"],
            "tools: [install_nightly]": ["--force", "--trust"],
            "tools: [read, edit, shell]\npermissions: read-only": [],
            "tools: [read]\npermissions: full-auto": ["--force", "--trust"],
            "tools: [read]\npermissions: typo": [],
            "tools:\n  - edit\n  - git_commit": ["--force", "--trust"],
        }
        for hdr, extra in cases.items():
            p = self.plan(f"Workspace: /w\n\n---\nid: GO-X\nmodel: m1\n{hdr}\n---\nB\n")
            self.assertEqual(p["args"], ["--model=m1"] + extra, hdr)
        self.assertEqual(self.plan("legacy prompt, no header\n")["args"], ["--model=grok-4.7-high"])

    def test_prompt_lookup_plans_dir_then_read_only_buzz(self):
        self.settings()
        slug = "_GO_ORBITTY_CG_PROMPT.md"
        (self.buzz / "PLANS").mkdir(parents=True)
        (self.buzz / "PLANS" / slug).write_text("old\n")
        before = self.buzz_snapshot()
        mod = load_plan(self.home)
        self.assertEqual(mod.prompt_for(GO), self.buzz / "PLANS" / slug)
        (self.plans / slug).write_text("new\n")
        self.assertEqual(mod.prompt_for(GO), self.plans / slug)
        self.assertEqual(self.buzz_snapshot(), before)


class CursorGoRun(HomeCase):
    """cursor-go for real (zsh), stub agent binary, argv captured."""

    def setUp(self):
        super().setUp()
        self.bin = self.home / ".local" / "bin"
        self.bin.mkdir(parents=True)
        self.argv_file = self.home / "argv.txt"
        agent = self.bin / "cursor-agent"
        agent.write_text(f'#!/bin/sh\nfor a in "$@"; do printf "%s\\n" "$a"; done > "{self.argv_file}.tmp"\nmv "{self.argv_file}.tmp" "{self.argv_file}"\n')
        agent.chmod(agent.stat().st_mode | stat.S_IEXEC)
        self.stubs = self.home / "stubs"
        self.stubs.mkdir()
        pgrep = self.stubs / "pgrep"
        pgrep.write_text("#!/bin/sh\nexit 1\n")
        pgrep.chmod(0o755)
        # cursor-go's python3 is the one running these tests (3.14 or the
        # watcher's /usr/bin/python3 3.9).
        (self.stubs / "python3").symlink_to(sys.executable)
        self.ws = self.home / "ws"
        self.ws.mkdir()

    def run_go(self, *args):
        env = {
            "HOME": str(self.home),
            "PATH": f"{self.stubs}:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin",
            "PYTHONDONTWRITEBYTECODE": "1",
        }
        return subprocess.run(["/bin/zsh", str(HERE / "cursor-go"), GO, *args],
                              env=env, capture_output=True, text=True, timeout=30)

    def agent_argv(self):
        for _ in range(100):
            if self.argv_file.is_file():
                return self.argv_file.read_text().splitlines()
            time.sleep(0.05)
        self.fail("stub cursor-agent never ran")

    def packet(self, header_body: str) -> Path:
        p = self.home / "packet.md"
        p.write_text(f"Workspace: {self.ws}\n\n{header_body}")
        return p

    def test_header_model_one_arg_and_force_for_full_auto(self):
        self.settings()
        r = self.run_go(str(self.packet(FULL)))
        self.assertEqual(r.returncode, 0, r.stderr)
        argv = self.agent_argv()
        self.assertIn("--model=grok-4.7-high", argv)
        self.assertNotIn("--model", argv)
        self.assertNotIn("kimi-k3-max", " ".join(argv))
        self.assertIn("--force", argv)
        self.assertEqual(argv[argv.index("--workspace") + 1], str(self.ws))
        self.assertIsNone(self.buzz_snapshot())

    def test_read_only_header_gets_no_force(self):
        self.settings(defaultRunner={"harness": "cursor", "model": "sonnet-4.5"})
        r = self.run_go(str(self.packet("---\nid: GO-ORBITTY-CG\ntools: [read]\n---\nB\n")))
        self.assertEqual(r.returncode, 0, r.stderr)
        argv = self.agent_argv()
        self.assertIn("--model=sonnet-4.5", argv)
        self.assertNotIn("--force", argv)
        self.assertNotIn("--trust", argv)

    def test_bad_model_refuses_and_agent_never_runs(self):
        self.settings()
        r = self.run_go(str(self.packet(FULL.replace("grok-4.7-high", "--yolo"))))
        self.assertEqual(r.returncode, 5, r.stdout + r.stderr)
        self.assertIn("invalid model", r.stderr)
        time.sleep(0.3)
        self.assertFalse(self.argv_file.exists())

    def test_prompt_from_plans_dir_nothing_under_buzz(self):
        self.settings()
        (self.plans / "_GO_ORBITTY_CG_PROMPT.md").write_text(f"Workspace: {self.ws}\n\n{FULL}")
        r = self.run_go()
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn(str(self.plans / "_GO_ORBITTY_CG_PROMPT.md"), r.stdout)
        self.assertIn("--model=grok-4.7-high", self.agent_argv())
        self.assertIsNone(self.buzz_snapshot())


if __name__ == "__main__":
    unittest.main()
