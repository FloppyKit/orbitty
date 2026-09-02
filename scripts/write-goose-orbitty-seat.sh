#!/bin/zsh
# GO-ORBITTY-MCP-SEAT — idempotent merge of extensions.orbitty into
# ~/.config/goose/config.yaml. Merge, never replace. Backup first.
set -euo pipefail

TARGET="${GOOSE_CONFIG:-$HOME/.config/goose/config.yaml}"
CMD="${ORBITTY_MCP_CMD:-$HOME/.local/bin/orbitty-mcp}"
STAMP=$(date +%Y%m%d-%H%M%S)
BACKUP="${TARGET}.bak-orbitty-${STAMP}"

ORBITTY_BLOCK=$(cat <<EOF
  orbitty:
    enabled: true
    type: stdio
    name: orbitty
    description: Drive a running Orbitty over its local control socket (windows, companion, packs, state)
    cmd: ${CMD}
    args: []
    envs: {}
    env_keys: []
    timeout: 360
    cwd: null
    bundled: null
EOF
)

mkdir -p "$(dirname "$TARGET")"

if [[ ! -f "$TARGET" ]]; then
  cat > "$TARGET" <<EOF
extensions:
${ORBITTY_BLOCK}
EOF
  echo "created $TARGET (no existing Goose config)"
  echo "orbitty seat: created"
  echo "cmd: $CMD"
  exit 0
fi

cp "$TARGET" "$BACKUP"
echo "backup: $BACKUP"

python3 - "$TARGET" "$BACKUP" "$CMD" "$ORBITTY_BLOCK" <<'PY'
import sys, re
from pathlib import Path

target, backup, cmd, block = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
p = Path(target)
text = p.read_text()

try:
    import yaml
    data = yaml.safe_load(text)
except Exception as e:
    Path(target).write_text(Path(backup).read_text())
    sys.stderr.write(f"FAIL: unreadable yaml ({e}); restored original from {backup}\n")
    sys.exit(1)

if data is None:
    data = {}
if not isinstance(data, dict):
    Path(target).write_text(Path(backup).read_text())
    sys.stderr.write(f"FAIL: yaml root is not a mapping; restored original from {backup}\n")
    sys.exit(1)

desired = {
    "enabled": True,
    "type": "stdio",
    "name": "orbitty",
    "description": "Drive a running Orbitty over its local control socket (windows, companion, packs, state)",
    "cmd": cmd,
    "args": [],
    "envs": {},
    "env_keys": [],
    "timeout": 360,
    "cwd": None,
    "bundled": None,
}

ext = data.get("extensions")
have = None
if isinstance(ext, dict):
    have = ext.get("orbitty")

def same(a, b):
    if not isinstance(a, dict):
        return False
    # Compare on the keys we care about; extra keys on disk are ok if core matches.
    for k, v in b.items():
        if a.get(k) != v:
            return False
    return True

# Surgical text replace — never dump the whole file (would scramble comments/order).
pat = re.compile(r"(?m)^  orbitty:\n(?:    .*\n)*")

if have is not None and same(have, desired) and pat.search(text):
    print("orbitty seat: already present (no rewrite)")
    print(f"cmd: {cmd}")
    sys.exit(0)

if pat.search(text):
    new = pat.sub(block.rstrip() + "\n", text, count=1)
    action = "updated"
elif re.search(r"(?m)^extensions:\s*$", text):
    new = re.sub(r"(?m)^extensions:\s*$", "extensions:\n" + block.rstrip(), text, count=1)
    action = "created"
elif re.search(r"(?m)^extensions:\s*\n", text):
    new = re.sub(r"(?m)^(extensions:\s*\n)", r"\1" + block.rstrip() + "\n", text, count=1)
    action = "created"
else:
    if text and not text.endswith("\n"):
        text += "\n"
    new = text + "\nextensions:\n" + block.rstrip() + "\n"
    action = "created"

p.write_text(new)

# Re-parse to prove we didn't break yaml.
try:
    again = yaml.safe_load(p.read_text())
    got = (again or {}).get("extensions", {}).get("orbitty")
except Exception as e:
    Path(target).write_text(Path(backup).read_text())
    sys.stderr.write(f"FAIL: merge produced unreadable yaml ({e}); restored {backup}\n")
    sys.exit(1)

if not same(got if isinstance(got, dict) else {}, desired):
    Path(target).write_text(Path(backup).read_text())
    sys.stderr.write("FAIL: merge did not land the orbitty block; restored\n")
    sys.exit(1)

print(f"orbitty seat: {action}")
print(f"cmd: {cmd}")
PY
