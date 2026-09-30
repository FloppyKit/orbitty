"""Harsha-style constrained classify for Orbitty Quick ack.

Not a chat model. Prefill once, slice logits over a closed verb set
(and a closed ack set). Weights come from ORBITTY_MOUTH_WEIGHTS or
~/.config/orbitty/mouth-sidecar.json — never a path baked into this
file. Apache-2.0 idea: harshatheg parallel constrained decode
(logit slice, not JSON-mode).
"""

from __future__ import annotations

import json
import os
import time
from typing import Any

VERBS = ["chat", "do-go", "run-action", "ask-needed", "none"]
HARNESSES = ["skip", "job", "pocket", "tight"]
ACKS = ["on_it", "heard", "one_sec", "hand_off", "your_call", "got_file", "nope"]
ACK_BY_VERB = {
    "chat": "on_it",
    "none": "heard",
    "do-go": "hand_off",
    "run-action": "nope",
    "ask-needed": "your_call",
}
# Skip the extra ack decode unless verb_ms leaves >=80 ms under the 200 ms gate.
ACK_ROOM_MS = 80.0
SWAP_IN_MS = 200.0

_NO_WEIGHTS = (
    "no weights configured (set ORBITTY_MOUTH_WEIGHTS or "
    "weights in ~/.config/orbitty/mouth-sidecar.json)"
)

_model = None
_tokenizer = None
_mx = None
_load_err: str | None = None


def _config_path() -> str | None:
    home = os.environ.get("HOME", "").strip()
    if not home:
        return None
    return os.path.join(home, ".config", "orbitty", "mouth-sidecar.json")


def _weights_from_config() -> str | None:
    path = _config_path()
    if not path:
        return None
    try:
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
    except (OSError, json.JSONDecodeError, UnicodeError):
        return None
    if not isinstance(data, dict):
        return None
    weights = data.get("weights")
    if not isinstance(weights, str):
        return None
    weights = weights.strip()
    return os.path.expanduser(weights) or None


def weights_path() -> str | None:
    env = os.environ.get("ORBITTY_MOUTH_WEIGHTS", "").strip()
    if env:
        return env
    return _weights_from_config()


def ready() -> bool:
    return _model is not None and _tokenizer is not None


def load_error() -> str | None:
    return _load_err


def load_model() -> None:
    global _model, _tokenizer, _mx, _load_err
    if _model is not None:
        return
    path = weights_path()
    if not path:
        _load_err = _NO_WEIGHTS
        return
    if not os.path.isdir(path):
        _load_err = f"weights missing: {path}"
        return
    try:
        import mlx.core as mx
        from mlx_lm import load

        _mx = mx
        _model, _tokenizer = load(path)
        _load_err = None
        # Warm one forward so the first classify is not the compile.
        _ = _score_choices('{"verb":"', VERBS)
    except Exception as e:  # noqa: BLE001 — sidecar must stay up
        _model = None
        _tokenizer = None
        _load_err = f"{type(e).__name__}: {e}"


def _encode_choice(text: str) -> list[int]:
    tok = _tokenizer
    ids = tok.encode(text, add_special_tokens=False)
    if isinstance(ids, tuple):
        ids = ids[0]
    return [int(i) for i in ids]


def _prompt(line: str) -> str:
    user = (line or "").replace("\n", " ").strip()
    if len(user) > 400:
        user = user[:399] + "…"
    messages = [
        {
            "role": "system",
            "content": (
                "You pick one Orbitty verb for the user line. "
                "chat=talk, the mouth answers — listing todos/the board is chat. "
                "none=noise/thanks. "
                "do-go=fire a GO or hand to Cursor. "
                "run-action=run a saved Action by name. "
                "ask-needed=Matt must decide (not a list question). "
                "Also pick wrap: skip=user line only, job=now.json, "
                "pocket=leftovers, tight=gist+history. No other words."
            ),
        },
        {"role": "user", "content": user or "(empty)"},
    ]
    tok = _tokenizer
    if hasattr(tok, "apply_chat_template"):
        return tok.apply_chat_template(
            messages, tokenize=False, add_generation_prompt=True
        )
    return f"System: {messages[0]['content']}\nUser: {user}\nAssistant: "


def _forward_last_logits(prefix: str):
    mx = _mx
    ids = _tokenizer.encode(prefix, add_special_tokens=False)
    if isinstance(ids, tuple):
        ids = ids[0]
    arr = mx.array([list(ids)])
    out = _model(arr)
    logits = out.logits if hasattr(out, "logits") else out
    # logits: [B, T, V]
    return logits[0, -1, :]


def _score_choices(prefix: str, choices: list[str]) -> list[tuple[str, float]]:
    mx = _mx
    last = _forward_last_logits(prefix)
    rows: list[tuple[str, Any]] = []
    for c in choices:
        ids = _encode_choice(c)
        if not ids:
            continue
        rows.append((c, last[ids[0]]))
    if not rows:
        return [(choices[0], 1.0)]
    z = mx.stack([r[1] for r in rows])
    p = mx.softmax(z.astype(mx.float32))
    mx.eval(p)
    out = [(rows[i][0], float(p[i])) for i in range(len(rows))]
    out.sort(key=lambda x: x[1], reverse=True)
    return out


def classify(line: str) -> dict[str, Any]:
    t0 = time.perf_counter()
    if not ready():
        load_model()
    if not ready():
        return {
            "ok": False,
            "error": load_error() or "not ready",
            "elapsed_ms": (time.perf_counter() - t0) * 1000.0,
        }
    base = _prompt(line)
    verbs = _score_choices(base + '{"verb":"', VERBS)
    verb, vprob = verbs[0]
    verb_ms = (time.perf_counter() - t0) * 1000.0
    ack = ACK_BY_VERB.get(verb, "on_it")
    if verb_ms <= (SWAP_IN_MS - ACK_ROOM_MS):
        try:
            acks = _score_choices(base + '{"ack":"', ACKS)
            if acks:
                ack = acks[0][0]
        except Exception:
            pass
    harness, hprob = "job", 0.0
    try:
        hs = _score_choices(base + '{"harness":"', HARNESSES)
        harness, hprob = hs[0]
    except Exception:
        pass
    elapsed = (time.perf_counter() - t0) * 1000.0
    return {
        "ok": True,
        "verb": verb,
        "prob": vprob,
        "ack": ack,
        "harness": harness,
        "harness_prob": hprob,
        "verb_ms": verb_ms,
        "elapsed_ms": elapsed,
        "top": [{"choice": c, "prob": p} for c, p in verbs[:3]],
    }
