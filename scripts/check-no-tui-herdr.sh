#!/bin/sh
# Read-side gate: herdr/hc commands stay inside src-tauri/src/herdr.
# find_on_path("herdr") and which("herdr") stay inside herdr/tui.rs.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
src="$root/src-tauri/src"
fail=0
python3 - "$src" <<'PY'
import os, sys
src = sys.argv[1]
bad = []
for dirpath, _, names in os.walk(src):
    for name in names:
        if not name.endswith(".rs"):
            continue
        path = os.path.join(dirpath, name)
        rel = os.path.relpath(path, src)
        text = open(path, encoding="utf-8", errors="replace").read()
        in_tui = rel == os.path.join("herdr", "tui.rs")
        in_herdr = rel.split(os.sep)[0] == "herdr"
        if not in_tui and ('find_on_path("herdr")' in text or 'which("herdr")' in text):
            bad.append("lookup outside tui.rs: " + rel)
        if not in_herdr and ('Command::new("herdr")' in text or 'Command::new("hc")' in text):
            bad.append("Command outside herdr/: " + rel)
if bad:
    print("\n".join(bad), file=sys.stderr)
    sys.exit(1)
PY
