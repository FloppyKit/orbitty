#!/bin/sh
# Full gate: herdr TUI commands stay inside src-tauri/src/herdr/tui.rs.
# find_on_path("herdr") and which("herdr") stay inside herdr/tui.rs.
# A mutant `run_json(&bin, &["pane","close",…])` outside tui.rs fails.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
src="$root/src-tauri/src"
python3 - "$src" <<'PY'
import os, re, sys
src = sys.argv[1]
needles = [
    '&["pane"',
    '&["workspace"',
    '&["agent"',
    '.args(["agent"',
    '.args(["pane"',
    '.args(["workspace"',
    '"pane","close"',
    '"workspace","close"',
    '"workspace","create"',
    '"workspace","focus"',
    '"workspace","list"',
    '"agent","prompt"',
    '"agent","wait"',
    '"agent","get"',
    '"agent","focus"',
    '"agent","read"',
]
bad = []
for dirpath, _, names in os.walk(src):
    for name in names:
        if not name.endswith(".rs"):
            continue
        path = os.path.join(dirpath, name)
        rel = os.path.relpath(path, src)
        text = open(path, encoding="utf-8", errors="replace").read()
        compact = re.sub(r"\s+", "", text)
        in_tui = rel == os.path.join("herdr", "tui.rs")
        in_herdr = rel.split(os.sep)[0] == "herdr"
        if not in_tui and ('find_on_path("herdr")' in text or 'which("herdr")' in text):
            bad.append("lookup outside tui.rs: " + rel)
        if not in_tui and ('Command::new("herdr")' in text or 'Command::new("hc")' in text):
            bad.append("Command outside tui.rs: " + rel)
        if not in_herdr and ('Command::new("herdr")' in text or 'Command::new("hc")' in text):
            bad.append("Command outside herdr/: " + rel)
        if not in_tui:
            for n in needles:
                if n in compact:
                    bad.append("tui argv outside tui.rs: " + rel + " " + n)
                    break
if bad:
    print("\n".join(bad), file=sys.stderr)
    sys.exit(1)
PY
