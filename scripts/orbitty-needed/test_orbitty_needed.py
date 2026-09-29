"""Tests for the GO-STANDARD parts of orbitty-needed (no Orbitty, no network).

Run: python3 -m unittest discover -s scripts/orbitty-needed -p 'test_*.py'

HOME points at a temp dir before the script loads, so every path constant
(BUS, PLANS, ...) lands there; orbittyctl is never called (ctl is stubbed).
"""
from __future__ import annotations

import atexit
import importlib.machinery
import importlib.util
import os
import shutil
import sys
import tempfile
import types
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
TMP_HOME = Path(tempfile.mkdtemp(prefix="orbitty-needed-test-"))
atexit.register(shutil.rmtree, TMP_HOME, True)
os.environ["HOME"] = str(TMP_HOME)
# The script imports two sibling helper modules; these tests never use them.
for name in ("orbitty_needed_rules", "orbitty_leftover_clerk"):
    sys.modules.setdefault(name, types.ModuleType(name))

_loader = importlib.machinery.SourceFileLoader("orbitty_needed", str(HERE / "orbitty-needed"))
_spec = importlib.util.spec_from_loader("orbitty_needed", _loader)
on = importlib.util.module_from_spec(_spec)
_loader.exec_module(on)

GO = "GO-ORBITTY-TEST"
CARD = "c-test-1"
NOTE = f"{GO} is ready, not sealed; not launching."


class Base(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(dir=TMP_HOME))
        self.plans = self.root / "Orbitty GOs"
        self.plans.mkdir()
        self.buzz = self.root / "buzz-PLANS"
        self.buzz.mkdir()
        self.bus = self.root / "bus"
        self.bus.mkdir()
        self.calls: list[tuple] = []
        self._saved = {}
        self.patch("PLANS", self.buzz)
        self.patch("BUS", self.bus)
        self.patch("PACKETS", self.bus / "go-packets")
        self.patch("DISPATCH_LOG", self.bus / "dispatch.log")
        self.patch("ISSUED", self.bus / "issued.json")
        self.patch("plans_dir", lambda: self.plans)
        self.patch("ctl", self.fake_ctl)
        self.patch("doing_fire_log", lambda msg: None)

    def tearDown(self):
        for k, v in self._saved.items():
            setattr(on, k, v)

    def patch(self, name, value):
        if name not in self._saved:
            self._saved[name] = getattr(on, name)
        setattr(on, name, value)

    def fake_ctl(self, *args, json_out=True):
        self.calls.append(args)
        return {}

    def doing_fire(self):
        rec = {
            "id": "n-1",
            "neededAnswer": "grok on this planet",
            "neededWhy": f"go={GO} card={CARD}",
            "projectId": "p-orbitty",
        }
        return on.dispatch_doing_fire(rec, "grok on this planet")


class DoingFireRefusal(Base):
    def test_refusal_appends_and_never_replaces_notes(self):
        self.patch("go_status", lambda **kw: {"sealed": False, "note": NOTE})
        self.assertTrue(self.doing_fire())
        notes = [c for c in self.calls if c[:2] == ("kanban", "notes")]
        self.assertEqual(notes, [("kanban", "notes", "--id", CARD, "--append", "--", NOTE)])

    def test_no_seal_check_leaves_notes_untouched(self):
        self.patch("go_status", lambda **kw: None)
        self.assertTrue(self.doing_fire())
        self.assertEqual([c for c in self.calls if c[:1] == ("kanban",)], [])


class PlansDirRouting(Base):
    def test_stamp_is_written_in_plans_dir_not_buzz(self):
        path = on.stamp_dogfood_green(GO, "c-1")
        self.assertEqual(path, self.plans / "STAMP_GO_ORBITTY_TEST.md")
        self.assertTrue(path.is_file())
        self.assertEqual(list(self.buzz.iterdir()), [])

    def test_existing_buzz_stamp_is_read_not_rewritten(self):
        old = self.buzz / "STAMP_GO_ORBITTY_TEST.md"
        old.write_text("clerk stamp\n")
        self.assertEqual(on.stamp_dogfood_green(GO, "c-1"), old)
        self.assertEqual(old.read_text(), "clerk stamp\n")
        self.assertEqual(list(self.plans.iterdir()), [])

    def test_leftover_lookup_reads_plans_dir(self):
        p = self.plans / "_GO_ORBITTY_TEST_LEFTOVER_1_PROMPT.md"
        p.write_text("leftover\n")
        self.assertEqual(on.leftover_prompt_for(GO), p)


class FullTextRoutes(Base):
    QUERY = "---\nid: GO-ORBITTY-TEST\n---\nBODY-SENTINEL\n\n--- lock /x/lock.md ---\n\nLOCK-SENTINEL\n\n## Launcher rules\n\n- git push: NOT allowed"

    def setUp(self):
        super().setUp()
        self.src = self.plans / "go.md"
        self.src.write_text("---\nid: GO-ORBITTY-TEST\n---\nBODY-SENTINEL {REPO_ROOT}\n")
        self.patch(
            "go_status",
            lambda **kw: {
                "sealed": True,
                "file": str(self.src),
                "workspace": "/ws/orbitty",
                "query": self.QUERY,
            },
        )
        self.patch("issued_status", lambda go, prompt=None: {"status": "none"})
        self.patch("issued_mark", lambda *a, **kw: {})

    def assert_packet(self, path: Path):
        self.assertNotEqual(path, self.src, "the raw GO file must not be handed over")
        text = path.read_text()
        self.assertTrue(text.startswith("Workspace: /ws/orbitty\n"), text)
        self.assertIn("LOCK-SENTINEL", text)
        self.assertIn("git push: NOT allowed", text)
        self.assertNotIn("{REPO_ROOT}", text)
        self.assertEqual(on.workspace_from_prompt(path), Path("/ws/orbitty"))

    def test_clerk_route_gets_lock_and_expanded_text(self):
        seen = {}
        self.patch("launch_grok_via_herdr", lambda: None)

        def fake_spawn(seat, packet, cid, sid):
            seen["packet"] = packet
            return types.SimpleNamespace(pid=4242)

        self.patch("spawn_clerk", fake_spawn)
        out = on.launch_writer("grok", GO, prompt=str(self.src))
        self.assertIn("has", out)
        self.assert_packet(Path(seen["packet"]))

    def test_cursor_go_route_gets_lock_and_expanded_text(self):
        seen = {}

        def fake_run(argv, **kw):
            seen["argv"] = argv
            return types.SimpleNamespace(returncode=0, stdout="cursor-go: pid 1\n", stderr="")

        self.patch("subprocess", types.SimpleNamespace(run=fake_run, Popen=None))
        on.launch_writer("cursor", GO, prompt=str(self.src))
        self.assertEqual(seen["argv"][1], GO)
        self.assert_packet(Path(seen["argv"][2]))

    def test_status_without_query_fails_closed(self):
        self.patch("go_status", lambda **kw: {"sealed": True, "file": str(self.src)})
        s = on.sealed_prompt(GO)
        self.assertIsNone(s.packet)
        self.assertIn("no GO text", s.note)

    def test_launch_note_refuses(self):
        self.patch(
            "go_status",
            lambda **kw: {"sealed": True, "file": str(self.src), "launchNote": "git_push refused"},
        )
        s = on.sealed_prompt(GO)
        self.assertIsNone(s.packet)
        self.assertIn("git_push refused", s.note)


if __name__ == "__main__":
    unittest.main()
