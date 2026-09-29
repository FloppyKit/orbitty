"""orbitty_needed_rules plans lookups with a throwaway HOME."""
from __future__ import annotations

import importlib.util
import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
GO = "GO-ORBITTY-RL"


class RulesPlans(unittest.TestCase):
    def setUp(self):
        self.home = Path(tempfile.mkdtemp(prefix="needed-rules-test-"))
        self.addCleanup(shutil.rmtree, self.home, True)
        self.plans = self.home / "Custom GOs"
        self.plans.mkdir()
        cfg = self.home / ".config" / "orbitty" / "settings.json"
        cfg.parent.mkdir(parents=True)
        cfg.write_text(json.dumps({"plansDir": str(self.plans)}))
        old = os.environ.get("HOME")
        os.environ["HOME"] = str(self.home)
        try:
            spec = importlib.util.spec_from_file_location(
                "orbitty_needed_rules_under_test", str(HERE / "orbitty_needed_rules.py"))
            self.m = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(self.m)
        finally:
            os.environ["HOME"] = old if old is not None else ""
        self.buzz = self.home / ".buzz"
        self.bus = self.home / "bus"
        self.bus.mkdir()
        self.m.BUS = self.bus  # the needed-bus is Matt's setting; keep it out of this test

    def snap(self):
        if not self.buzz.exists():
            return None
        return sorted((str(p), p.read_bytes() if p.is_file() else b"") for p in self.buzz.rglob("*"))

    def test_first_prompt_reads_plans_dir(self):
        p = self.plans / "_GO_ORBITTY_RL_PROMPT.md"
        p.write_text("first\n")
        self.assertEqual(self.m.first_prompt_for(GO), p)
        self.assertIsNone(self.snap())

    def test_leftover_reads_plans_dir(self):
        p = self.plans / "_GO_ORBITTY_RL_LEFTOVER_X_PROMPT.md"
        p.write_text("left\n")
        self.assertEqual(self.m.leftover_prompt_for(GO), p)
        self.assertIsNone(self.snap())

    def test_buzz_is_a_read_only_fallback(self):
        old = self.buzz / "PLANS"
        old.mkdir(parents=True)
        (old / "_GO_ORBITTY_RL_PROMPT.md").write_text("old first\n")
        (old / "_GO_ORBITTY_RL_LEFTOVER_Y_PROMPT.md").write_text("old left\n")
        before = self.snap()
        self.assertEqual(self.m.first_prompt_for(GO), old / "_GO_ORBITTY_RL_PROMPT.md")
        self.assertEqual(self.m.leftover_prompt_for(GO), old / "_GO_ORBITTY_RL_LEFTOVER_Y_PROMPT.md")
        new = self.plans / "_GO_ORBITTY_RL_PROMPT.md"
        new.write_text("new\n")
        self.assertEqual(self.m.first_prompt_for(GO), new)
        self.assertEqual(self.snap(), before)

    def test_default_plans_dir_is_visible_home_folder(self):
        (self.home / ".config" / "orbitty" / "settings.json").write_text("{}")
        self.assertEqual(self.m.plans_dir(), self.home / "Orbitty GOs")
        self.assertFalse((self.home / "Orbitty GOs").exists(), "a read never creates the folder")


if __name__ == "__main__":
    unittest.main()
