#!/bin/zsh
# Kitchen venv for the 1B mouth sidecar. Weights come from the env or ~/.config/orbitty/mouth-sidecar.json.
set -euo pipefail
cd "$(dirname "$0")"
PY="${ORBITTY_MOUTH_PYTHON:-$HOME/.local/bin/python3.12}"
if [[ ! -x "$PY" ]]; then
  PY="$(command -v python3.12 || true)"
fi
if [[ -z "$PY" || ! -x "$PY" ]]; then
  echo "need python3.12 for mlx (got none)" >&2
  exit 1
fi
if [[ ! -d .venv ]]; then
  echo "==> venv $PY"
  "$PY" -m venv .venv
fi
.venv/bin/pip install -U pip
.venv/bin/pip install -r requirements.txt
echo "ok sidecar/mouth venv"
echo "weights: ${ORBITTY_MOUTH_WEIGHTS:-not set (see ~/.config/orbitty/mouth-sidecar.json)}"
