"""orbitty_needed_hc_status: flag on uses hc server status, flag off stays herdr status."""
import importlib.machinery
import importlib.util
import unittest
from pathlib import Path


def load():
    path = Path(__file__).with_name("orbitty-needed")
    loader = importlib.machinery.SourceFileLoader("orbitty_needed", str(path))
    spec = importlib.util.spec_from_loader("orbitty_needed", loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


class HcStatus(unittest.TestCase):
    def test_orbitty_needed_hc_status(self):
        mod = load()

        class Proc:
            def __init__(self, stdout, rc):
                self.stdout = stdout
                self.returncode = rc

        def ctl_on(*_a):
            return [{"id": "herdr_core", "on": True}]

        def ctl_off(*_a):
            return [{"id": "herdr_core", "on": False}]

        seen = []

        def run_hc(argv, **_k):
            seen.append(argv)
            return Proc('{"version":"0.1.14","live":0}\n', 0)

        self.assertTrue(mod._herdr_server_up(ctl_on, run_hc))
        self.assertEqual(seen[-1][:3], ["hc", "server", "status"])

        def run_err(argv, **_k):
            return Proc('{"error":{"kind":"unavailable"}}\n', 1)

        self.assertFalse(mod._herdr_server_up(ctl_on, run_err))
        self.assertFalse(mod._hc_status_up('{"version":"0.1.14"}', 3))

        def run_herdr(argv, **_k):
            seen.append(argv)
            return Proc("status: running\n", 0)

        self.assertTrue(mod._herdr_server_up(ctl_off, run_herdr))
        self.assertEqual(seen[-1], ["herdr", "status"])


if __name__ == "__main__":
    unittest.main()
