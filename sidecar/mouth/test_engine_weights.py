"""weights_path() is env, then config, then None. No baked-in disk."""

from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path

import engine

NO_WEIGHTS = (
    "no weights configured (set ORBITTY_MOUTH_WEIGHTS or "
    "weights in ~/.config/orbitty/mouth-sidecar.json)"
)


class WeightsPathTests(unittest.TestCase):
    def setUp(self):
        self._old_env = os.environ.get("ORBITTY_MOUTH_WEIGHTS")
        self._old_home = os.environ.get("HOME")
        os.environ.pop("ORBITTY_MOUTH_WEIGHTS", None)
        self.tmp = tempfile.TemporaryDirectory()
        os.environ["HOME"] = self.tmp.name
        engine._model = None
        engine._tokenizer = None
        engine._mx = None
        engine._load_err = None

    def tearDown(self):
        if self._old_env is None:
            os.environ.pop("ORBITTY_MOUTH_WEIGHTS", None)
        else:
            os.environ["ORBITTY_MOUTH_WEIGHTS"] = self._old_env
        if self._old_home is None:
            os.environ.pop("HOME", None)
        else:
            os.environ["HOME"] = self._old_home
        self.tmp.cleanup()
        engine._model = None
        engine._tokenizer = None
        engine._mx = None
        engine._load_err = None

    def _write(self, obj):
        cfg = Path(self.tmp.name) / ".config" / "orbitty"
        cfg.mkdir(parents=True)
        text = obj if isinstance(obj, str) else json.dumps(obj)
        (cfg / "mouth-sidecar.json").write_text(text, encoding="utf-8")

    def test_env_then_config_then_none(self):
        self.assertIsNone(engine.weights_path())
        self._write({"url": "http://127.0.0.1:13121", "weights": "/from/config"})
        self.assertEqual(engine.weights_path(), "/from/config")
        os.environ["ORBITTY_MOUTH_WEIGHTS"] = "/from/env"
        self.assertEqual(engine.weights_path(), "/from/env")

    def test_config_tilde_expands_home(self):
        self._write({"weights": "~/models/x"})
        self.assertEqual(
            engine.weights_path(),
            os.path.join(self.tmp.name, "models", "x"),
        )

    def test_malformed_config_is_none(self):
        self._write("{")
        self.assertIsNone(engine.weights_path())

    def test_load_model_no_weights_sets_error_and_does_not_raise(self):
        engine.load_model()
        self.assertIsNone(engine._model)
        self.assertFalse(engine.ready())
        self.assertEqual(engine.load_error(), NO_WEIGHTS)


if __name__ == "__main__":
    unittest.main()
