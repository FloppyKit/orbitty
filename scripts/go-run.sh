#!/bin/zsh
# Conductor for one Go run. Chuck copies this to ~/bin/go-run.sh after PASS.
# Usage: go-run.sh <card_id> <go_file> <run_id>
# Stubs: GO_RUN_CTL GO_RUN_AUDIT GO_RUN_INSTALL GO_RUN_MATCH GO_RUN_OPEN GO_RUN_HOME
# The cook never runs this against the live app. Stubs only.
set -uo pipefail

if [[ -n ${GO_RUN_HOME:-} ]]; then
  export HOME="$GO_RUN_HOME"
fi

CARD=${1:-}
GOFILE=${2:-}
RUN_ID=${3:-}
CTL=${GO_RUN_CTL:-orbittyctl}
DISK_MIN_KIB=21495808
PLANS="$HOME/Orbitty GOs"
RUNS="$PLANS/_go-runs"
LOCK="$RUNS/LOCK"
LOCK_HELD=0
FINISHED=0
STAGE=""
GO_ID=""
WT=""
BASE=""
TIP=""
ROUND=0
RUNNER_ID=""
COOK=""
VERIFY=""
PERMS_JSON='{"allow_push":false,"allow_merge_main":false,"allow_stable_install":false,"allow_send":false}'
STARTED=""
RESULT_LINE=""
LOGS_JSON='{}'
SNAP=""

die_usage() {
  print -r -- "usage: go-run.sh <card_id> <go_file> <run_id>" >&2
  exit 2
}

[[ -n $CARD && -n $GOFILE && -n $RUN_ID ]] || die_usage
case $RUN_ID in
  *[!A-Za-z0-9._-]*) die_usage ;;
esac
[[ -f $GOFILE ]] || { print -r -- "BLOCKED: GO missing file $GOFILE"; exit 1; }

RUN_DIR="$RUNS/$RUN_ID"
STATE="$RUN_DIR/state.json"
mkdir -p "$RUN_DIR" "$PLANS/_logs"

now() { date -u +%Y-%m-%dT%H:%M:%SZ; }
STARTED=$(now)

header_val() {
  local key=$1
  awk -v k="$key" '
    BEGIN { f = 0 }
    /^---[[:space:]]*$/ { f++; if (f == 2) exit; next }
    f == 1 && index($0, k ":") == 1 {
      sub("^" k ":[[:space:]]*", "")
      gsub(/^["'\'']|["'\'']$/, "")
      gsub(/[[:space:]]+$/, "")
      print
      exit
    }
  ' "$GOFILE"
}

GO_ID=$(header_val id)
BASE=$(header_val base)
BRANCH=$(header_val branch)
WT=$(header_val worktree)
[[ -n $GO_ID ]] || GO_ID=GO

write_state() {
  UPDATED=$(now)
  STATE_PATH="$STATE" RUN_ID="$RUN_ID" CARD="$CARD" GO_ID="$GO_ID" BASE="$BASE" TIP="$TIP" \
    ROUND="$ROUND" RUNNER_ID="$RUNNER_ID" STAGE="$STAGE" RESULT_LINE="$RESULT_LINE" \
    STARTED="$STARTED" UPDATED="$UPDATED" PERMS_JSON="$PERMS_JSON" LOGS_JSON="$LOGS_JSON" \
    python3 - <<'PY'
import json, os
p = os.environ["STATE_PATH"]
doc = {
    "run_id": os.environ.get("RUN_ID", ""),
    "card_id": os.environ.get("CARD", ""),
    "go": os.environ.get("GO_ID", ""),
    "base": os.environ.get("BASE", ""),
    "tip": os.environ.get("TIP", ""),
    "round": int(os.environ.get("ROUND") or "0"),
    "runner": os.environ.get("RUNNER_ID", ""),
    "permissions": json.loads(os.environ.get("PERMS_JSON") or "{}"),
    "stage": os.environ.get("STAGE", ""),
    "result_line": os.environ.get("RESULT_LINE", ""),
    "started_at": os.environ.get("STARTED", ""),
    "updated_at": os.environ.get("UPDATED", ""),
    "logs": json.loads(os.environ.get("LOGS_JSON") or "{}"),
}
tmp = p + ".tmp"
with open(tmp, "w") as f:
    json.dump(doc, f, indent=2)
    f.write("\n")
os.replace(tmp, p)
PY
}

release_lock() {
  if (( LOCK_HELD )); then
    rm -rf -- "$LOCK"
    LOCK_HELD=0
  fi
}

note() {
  local line=$1
  if "$CTL" kanban notes --id "$CARD" --append -- "$line" >>"$RUN_DIR/ctl.out" 2>&1; then
    return 0
  fi
  print -r -- "$line" >> "$RUN_DIR/pending-notes"
  return 0
}

flush_notes() {
  [[ -f $RUN_DIR/pending-notes ]] || return 0
  local next=$RUN_DIR/pending-notes.next line
  : > "$next"
  while IFS= read -r line; do
    if ! "$CTL" kanban notes --id "$CARD" --append -- "$line" >>"$RUN_DIR/ctl.out" 2>&1; then
      print -r -- "$line" >> "$next"
    fi
  done < "$RUN_DIR/pending-notes"
  if [[ -s $next ]]; then
    mv -f "$next" "$RUN_DIR/pending-notes"
  else
    rm -f "$next" "$RUN_DIR/pending-notes"
  fi
}

webhook() {
  [[ -n ${GO_DONE_WEBHOOK_URL:-} ]] || return 0
  local hdr=()
  if [[ -n ${GO_DONE_WEBHOOK_KEY:-} ]]; then
    hdr=(-H "X-Go-Done-Key: ${GO_DONE_WEBHOOK_KEY}")
  fi
  curl -fsS -m 10 "${hdr[@]}" -H 'Content-Type: application/json' --data-binary @"$STATE" "$GO_DONE_WEBHOOK_URL" >/dev/null 2>>"$RUN_DIR/webhook.err" || note "GO run $RUN_ID: webhook failed"
}

finish_blocked() {
  local reason=$1
  FINISHED=1
  STAGE=blocked
  RESULT_LINE="BLOCKED: $GO_ID $reason"
  write_state || true
  note "GO run $RUN_ID: blocked: $reason"
  flush_notes
  release_lock
  webhook
  print -r -- "$RESULT_LINE"
  exit 1
}

on_exit() {
  local rc=$?
  if (( FINISHED )); then
    release_lock
    return
  fi
  if (( rc != 0 )); then
    STAGE=blocked
    RESULT_LINE="BLOCKED: $GO_ID conductor exited $rc"
    write_state || true
    note "GO run $RUN_ID: blocked: conductor exited $rc" || true
  fi
  release_lock
}
trap on_exit EXIT

disk_kib() {
  # GO_RUN_HOME is the stub seam. A missing .df-kib means "disk is fine"
  # so a low live volume cannot fail the stub cases. Real runs read df.
  if [[ -n ${GO_RUN_HOME:-} ]]; then
    if [[ -f $HOME/.df-kib ]]; then
      tr -d '[:space:]' < "$HOME/.df-kib"
    else
      print -r -- 999999999
    fi
    return 0
  fi
  /bin/df -k / | awk 'NR==2 { print $4 }'
}

check_disk() {
  local kib
  kib=$(disk_kib) || finish_blocked "disk check failed"
  [[ $kib == <-> ]] || finish_blocked "disk check failed"
  if (( kib < DISK_MIN_KIB )); then
    finish_blocked "disk ${kib} KiB free, need ${DISK_MIN_KIB}"
  fi
}

check_rig_ok() {
  local f
  for f in "$HOME"/rgl-proofs/*/logs/RIG_OK(N); do
    [[ -e $f ]] || continue
    finish_blocked "RIG_OK present ($f); not starting"
  done
}

proc_lines() {
  if [[ -n ${GO_RUN_HOME:-} ]]; then
    [[ -f $HOME/.procs ]] && cat "$HOME/.procs"
    return 0
  fi
  /bin/ps -ax -o command=
}

check_heavy() {
  local line base next low i
  local -a parts
  while IFS= read -r line; do
    [[ -n $line ]] || continue
    if [[ $line == *vizdoom* ]]; then
      finish_blocked "heavy job running (vizdoom)"
    fi
    low=${line:l}
    if [[ $low == *python* && ($low == *rgl-proofs* || $low == *vizdoom*) ]]; then
      finish_blocked "heavy job running (RGL python)"
    fi
    parts=(${=line})
    for (( i = 1; i <= ${#parts}; i++ )); do
      base=${parts[i]:t}
      if [[ $base == cargo ]]; then
        next=${parts[i+1]:-}
        case $next in
          build|test|check|run|clippy|install|bench)
            finish_blocked "heavy job running (cargo $next)"
            ;;
        esac
      fi
    done
  done < <(proc_lines)
}

validate_prog() {
  local p=$1 label=$2 rc=0
  if [[ -z $p || $p == *[[:space:]]* || $p == -* ]]; then
    finish_blocked "runner config: $label is not a single path"
  fi
  if [[ -L $p || ! -f $p ]]; then
    finish_blocked "runner config: $label is not a regular file"
  fi
  python3 - "$p" <<'PY' || rc=$?
import os, stat, sys
st = os.lstat(sys.argv[1])
if stat.S_ISLNK(st.st_mode) or not stat.S_ISREG(st.st_mode):
    sys.exit(2)
if st.st_uid != os.getuid():
    sys.exit(3)
if st.st_mode & 0o022:
    sys.exit(4)
if not (st.st_mode & 0o111):
    sys.exit(5)
PY
  case $rc in
    0) ;;
    3) finish_blocked "runner config: $label is not owned by the user" ;;
    4) finish_blocked "runner config: $label is group or world writable" ;;
    5) finish_blocked "runner config: $label is not executable" ;;
    *) finish_blocked "runner config: $label is not runnable" ;;
  esac
}

load_runner() {
  local raw rc=0
  raw=$("$CTL" --json go runner 2>>"$RUN_DIR/ctl.out") || rc=$?
  if (( rc != 0 )); then
    PERMS_JSON='{"allow_push":false,"allow_merge_main":false,"allow_stable_install":false,"allow_send":false}'
    RUNNER_ID=pi-grok
    COOK="$HOME/bin/cook-pi.sh"
    VERIFY="$HOME/bin/verify-pi.sh"
    return 0
  fi
  RUNNER_JSON=$raw python3 - <<'PY' > "$RUN_DIR/runner.parsed"
import json, os, sys
raw = os.environ.get("RUNNER_JSON", "")
try:
    d = json.loads(raw)
except Exception as e:
    print("ERR")
    print(f"parse {e}")
    sys.exit(0)
err = d.get("error") or ""
if err:
    print("ERR")
    print(err)
    sys.exit(0)
perms = d.get("permissions") or {}
for k in ("allow_push", "allow_merge_main", "allow_stable_install", "allow_send"):
    if k not in perms:
        perms[k] = False
    perms[k] = bool(perms[k])
print("OK")
print(d.get("runner") or "")
print(d.get("cook") or "")
print(d.get("verify") or "")
print(json.dumps({k: bool(perms[k]) for k in ("allow_push", "allow_merge_main", "allow_stable_install", "allow_send")}, separators=(",", ":")))
PY
  local kind msg
  kind=$(sed -n '1p' "$RUN_DIR/runner.parsed")
  if [[ $kind == ERR ]]; then
    msg=$(sed -n '2p' "$RUN_DIR/runner.parsed")
    finish_blocked "runner config: $msg"
  fi
  RUNNER_ID=$(sed -n '2p' "$RUN_DIR/runner.parsed")
  COOK=$(sed -n '3p' "$RUN_DIR/runner.parsed")
  VERIFY=$(sed -n '4p' "$RUN_DIR/runner.parsed")
  PERMS_JSON=$(sed -n '5p' "$RUN_DIR/runner.parsed")
  [[ -n $RUNNER_ID && -n $COOK && -n $VERIFY ]] || finish_blocked "runner config: incomplete runner reply"
}

seal_ok() {
  local raw rc=0
  raw=$("$CTL" --json go status --file "$GOFILE" 2>>"$RUN_DIR/ctl.out") || rc=$?
  if (( rc != 0 )); then
    finish_blocked "cannot confirm seal (orbittyctl exit $rc)"
  fi
  local msg rc=0
  msg=$(SEAL_JSON=$raw python3 - <<'PY'
import json, os, sys
d = json.loads(os.environ.get("SEAL_JSON") or "{}")
note = (d.get("note") or "").strip()
if d.get("sealed") is True and note == "":
    sys.exit(0)
print(note or "GO is not sealed")
sys.exit(1)
PY
) || rc=$?
  if (( rc != 0 )); then
    finish_blocked "${msg:-GO is not sealed}"
  fi
}

worktree_ok() {
  [[ -n $WT && -n $BASE ]] || finish_blocked "GO header missing base or worktree"
  if [[ ! -e $WT ]]; then
    [[ -n $BRANCH ]] || finish_blocked "GO header missing branch"
    local repo ws
    ws=$(header_val workspace)
    ws=${ws//\{REPO_ROOT\}/$HOME/orbitty}
    if [[ -n $ws ]] && git -C "$ws" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
      repo=$ws
    elif git -C "$HOME/orbitty" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
      repo=$HOME/orbitty
    else
      finish_blocked "worktree absent and no repo to add it from"
    fi
    if git -C "$repo" show-ref --verify --quiet "refs/heads/$BRANCH"; then
      git -C "$repo" worktree add "$WT" "$BRANCH" >>"$RUN_DIR/worktree.log" 2>&1 \
        || finish_blocked "worktree add failed"
    else
      git -C "$repo" worktree add -b "$BRANCH" "$WT" "$BASE" >>"$RUN_DIR/worktree.log" 2>&1 \
        || finish_blocked "worktree add failed"
    fi
  fi
  local head dirty
  head=$(git -C "$WT" rev-parse HEAD 2>/dev/null) || finish_blocked "worktree has no HEAD"
  if [[ $head != $BASE ]]; then
    finish_blocked "base $BASE != worktree HEAD $head"
  fi
  dirty=$(git -C "$WT" status --porcelain 2>/dev/null || true)
  if [[ -n $dirty ]]; then
    finish_blocked "worktree is dirty"
  fi
  TIP=$head
}

take_lock() {
  mkdir -p "$RUNS"
  if ! mkdir "$LOCK" 2>/dev/null; then
    finish_blocked "another Go run holds the lock"
  fi
  LOCK_HELD=1
  print -r -- "$CARD" > "$LOCK/card"
}

perm_on() {
  local key=$1
  PERMS_JSON=$PERMS_JSON python3 - "$key" <<'PY'
import json, os, sys
d = json.loads(os.environ.get("PERMS_JSON") or "{}")
sys.exit(0 if d.get(sys.argv[1]) else 1)
PY
}

snapshot() {
  SNAP=$RUN_DIR/snapshot
  {
    local main_sha up
    main_sha=$(git -C "$WT" rev-parse --verify main 2>/dev/null || print NONE)
    print -r -- "main $main_sha"
    up=$(git -C "$WT" rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' 2>/dev/null || true)
    [[ -n $up ]] || up=NONE
    print -r -- "upstream $up"
    print -r -- "remotes"
    git -C "$WT" for-each-ref --format='%(objectname) %(refname)' refs/remotes 2>/dev/null || true
    print -r -- "stable"
    local n f dir=$HOME/Applications/Orbitty.app/Contents/MacOS
    for n in orbitty orbittyctl orbitty-mcp orbitty-relay; do
      f=$dir/$n
      if [[ -f $f ]]; then
        print -r -- "$n $(shasum -a 256 "$f" | awk '{print $1}')"
      else
        print -r -- "$n MISSING"
      fi
    done
  } > "$SNAP"
}

tripwire() {
  local cur=$RUN_DIR/snapshot.now
  local main_sha up
  main_sha=$(git -C "$WT" rev-parse --verify main 2>/dev/null || print NONE)
  print -r -- "main $main_sha" > "$cur"
  up=$(git -C "$WT" rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' 2>/dev/null || true)
  [[ -n $up ]] || up=NONE
  print -r -- "upstream $up" >> "$cur"
  print -r -- "remotes" >> "$cur"
  git -C "$WT" for-each-ref --format='%(objectname) %(refname)' refs/remotes 2>/dev/null >> "$cur" || true
  print -r -- "stable" >> "$cur"
  local n f dir=$HOME/Applications/Orbitty.app/Contents/MacOS
  for n in orbitty orbittyctl orbitty-mcp orbitty-relay; do
    f=$dir/$n
    if [[ -f $f ]]; then
      print -r -- "$n $(shasum -a 256 "$f" | awk '{print $1}')" >> "$cur"
    else
      print -r -- "$n MISSING" >> "$cur"
    fi
  done
  python3 - "$SNAP" "$cur" <<'PY'
import sys
def load(path):
    main = upstream = None
    remotes = {}
    stable = {}
    section = None
    for line in open(path):
        line = line.rstrip("\n")
        if line.startswith("main "):
            main = line.split(" ", 1)[1]
        elif line.startswith("upstream "):
            upstream = line.split(" ", 1)[1]
        elif line == "remotes":
            section = "remotes"
        elif line == "stable":
            section = "stable"
        elif section == "remotes" and line:
            sha, ref = line.split(" ", 1)
            remotes[ref] = sha
        elif section == "stable" and line:
            name, sha = line.split(" ", 1)
            stable[name] = sha
    return main, upstream, remotes, stable
a = load(sys.argv[1])
b = load(sys.argv[2])
reasons = []
if a[0] != b[0]:
    reasons.append("main")
if a[1] != b[1] or a[2] != b[2]:
    reasons.append("push")
if a[3] != b[3]:
    reasons.append("stable")
print(" ".join(reasons))
PY
}

apply_tripwire() {
  local why
  why=$(tripwire)
  [[ -n $why ]] || return 0
  local part
  for part in ${=why}; do
    case $part in
      push)
        if perm_on allow_push; then
          note "GO run $RUN_ID: tripwire allow_push: remote ref changed (allowed)"
        else
          finish_blocked "tripwire: new remote ref (allow_push is off)"
        fi
        ;;
      main)
        if perm_on allow_merge_main; then
          note "GO run $RUN_ID: tripwire allow_merge_main: main moved (allowed)"
        else
          finish_blocked "tripwire: main moved (allow_merge_main is off)"
        fi
        ;;
      stable)
        if perm_on allow_stable_install; then
          note "GO run $RUN_ID: tripwire allow_stable_install: Stable binaries changed (allowed)"
        else
          finish_blocked "tripwire: Stable binaries changed (allow_stable_install is off)"
        fi
        ;;
    esac
  done
}

last_line() {
  local f=$1
  [[ -s $f ]] || return 0
  tail -n 1 "$f"
}

# Last line matching re. Wrappers append "EXIT n" after the verdict.
verdict_line() {
  local f=$1 re=$2
  [[ -s $f ]] || return 0
  grep -E "$re" "$f" | tail -n 1
}

# Exit code printed after the verdict line. Empty if the wrapper wrote none.
exit_after_verdict() {
  local f=$1 verdict=$2
  python3 - "$f" "$verdict" <<'PY'
import sys
path, verdict = sys.argv[1], sys.argv[2]
try:
    lines = open(path, errors="replace").read().splitlines()
except OSError:
    sys.exit(0)
idx = None
for i, line in enumerate(lines):
    if line == verdict:
        idx = i
if idx is None:
    sys.exit(0)
code = None
for line in lines[idx + 1:]:
    if line.startswith("EXIT "):
        bits = line.split()
        if len(bits) >= 2 and bits[1].lstrip("-").isdigit():
            code = bits[1]
if code is not None:
    print(code)
PY
}

# Plans dir has a space, so the report is not one token. Last absolute
# span that ends in .md.
report_from_line() {
  python3 - "$1" <<'PY'
import sys
parts = sys.argv[1].split()
found = ""
i = 0
while i < len(parts):
    t = parts[i]
    if t.startswith("/") or t.startswith("file://"):
        buf = [t]
        j = i
        while j + 1 < len(parts) and not buf[-1].endswith(".md"):
            j += 1
            buf.append(parts[j])
        if buf[-1].endswith(".md"):
            found = " ".join(buf)
        i = j + 1
    else:
        i += 1
print(found)
PY
}

card_project() {
  local raw rc=0
  raw=$("$CTL" --json state get 2>>"$RUN_DIR/ctl.out") || rc=$?
  if (( rc != 0 )); then
    finish_blocked "card project unreadable (state get exit $rc)"
  fi
  STATE_JSON=$raw CARD=$CARD python3 - <<'PY'
import json, os, sys
raw = os.environ.get("STATE_JSON") or ""
try:
    d = json.loads(raw)
except Exception as e:
    print(f"bad state json: {e}")
    sys.exit(1)
cards = d.get("kanbanCards") or d.get("kanban_cards") or []
want = os.environ.get("CARD", "")
for c in cards:
    if c.get("id") == want:
        print(c.get("projectId") or c.get("project_id") or "")
        sys.exit(0)
sys.exit(2)
PY
}

dogfood_why() {
  python3 - "$1" <<'PY'
import sys
path = sys.argv[1]
try:
    lines = open(path, errors="replace").read().splitlines()
except OSError:
    print("See the Verify report.")
    sys.exit(0)
out = []
on = False
for line in lines:
    if line.startswith("## DOGFOOD (Matt)"):
        on = True
        continue
    if on and line.startswith("## "):
        break
    if on:
        out.append(line)
text = "\n".join(out).strip()
if not text:
    text = "See the Verify report."
if len(text) > 1500:
    text = text[:1500].rstrip() + "\n\u2026full list in the report"
print(text)
PY
}

file_url_for() {
  python3 -c 'from urllib.parse import quote; import sys; print("file://" + quote(sys.argv[1]))' "$1"
}

installed_commit() {
  if [[ -n ${GO_RUN_HOME:-} ]]; then
    if [[ -f $HOME/.installed-commit ]]; then
      tr -d '[:space:]' < "$HOME/.installed-commit"
    else
      print -r -- "$TIP"
    fi
    return 0
  fi
  /usr/libexec/PlistBuddy -c 'Print :OrbittyGitCommit' \
    "$HOME/Applications/Orbitty Nightly.app/Contents/Info.plist" 2>/dev/null || true
}

log_set() {
  local key=$1 val=$2
  LOGS_JSON=$(LOGS_JSON=$LOGS_JSON python3 -c 'import json,os,sys; d=json.loads(os.environ["LOGS_JSON"]); d[sys.argv[1]]=sys.argv[2]; print(json.dumps(d))' "$key" "$val")
}

run_cook() {
  local extra=${1:-} log=$RUN_DIR/cook-$ROUND.log
  STAGE=cooking
  if (( ROUND > 0 )); then
    STAGE=fixing
  fi
  write_state
  if [[ -n $extra ]]; then
    print -r -- "cook $COOK $GO_ID $WT $GOFILE $extra" >> "$RUN_DIR/argv.log"
    "$COOK" "$GO_ID" "$WT" "$GOFILE" "$extra" > "$log" 2>&1 || true
  else
    print -r -- "cook $COOK $GO_ID $WT $GOFILE" >> "$RUN_DIR/argv.log"
    "$COOK" "$GO_ID" "$WT" "$GOFILE" > "$log" 2>&1 || true
  fi
  log_set cook "$log"
  local line code
  line=$(verdict_line "$log" '^(DONE|BLOCKED|RIG BLOCKER):')
  RESULT_LINE=$line
  code=$(exit_after_verdict "$log" "$line")
  if [[ $line == "RIG BLOCKER:"* ]]; then
    finish_blocked "cook: $line"
  fi
  if [[ $line != "DONE:"* ]]; then
    finish_blocked "cook: ${line:-no DONE line}"
  fi
  if [[ -n $code && $code != 0 ]]; then
    finish_blocked "cook exit $code"
  fi
  TIP=$(git -C "$WT" rev-parse HEAD)
}

run_verify() {
  local log=$RUN_DIR/verify-$ROUND.log
  STAGE=verifying
  write_state
  note "GO run $RUN_ID: verifying (round $ROUND)"
  print -r -- "verify $VERIFY $GO_ID $TIP $WT $GOFILE" >> "$RUN_DIR/argv.log"
  "$VERIFY" "$GO_ID" "$TIP" "$WT" "$GOFILE" > "$log" 2>&1 || true
  log_set verify "$log"
  local line code
  line=$(verdict_line "$log" '^RESULT:')
  RESULT_LINE=$line
  code=$(exit_after_verdict "$log" "$line")
  if [[ -n $line && -n $code && $code != 0 ]]; then
    finish_blocked "verify exit $code"
  fi
}

procs_have() {
  local needle=$1 line
  while IFS= read -r line; do
    [[ $line == *$needle* ]] && return 0
  done < <(proc_lines)
  return 1
}

relaunch() {
  STAGE=relaunching
  write_state
  if procs_have "Orbitty.app/Contents/MacOS/orbitty" && ! procs_have "Orbitty Nightly.app/Contents/MacOS/orbitty"; then
    note "GO run $RUN_ID: Stable is running; not quitting it"
    return 0
  fi
  if ! procs_have "Orbitty Nightly.app/Contents/MacOS/orbitty"; then
    note "GO run $RUN_ID: Nightly is not running; skipped relaunch"
    return 0
  fi
  note "GO run $RUN_ID: relaunching Nightly"
  if [[ -n ${GO_RUN_HOME:-} ]]; then
    print -r -- "quit Orbitty Nightly" >> "$HOME/.quit-log"
  else
    osascript -e 'quit app "Orbitty Nightly"' >>"$RUN_DIR/relaunch.log" 2>&1 || true
    local i
    for (( i = 0; i < 30; i++ )); do
      procs_have "Orbitty Nightly.app/Contents/MacOS/orbitty" || break
      sleep 1
    done
    if procs_have "Orbitty Nightly.app/Contents/MacOS/orbitty"; then
      finish_blocked "Nightly did not exit within 30s"
    fi
  fi
  local opener=${GO_RUN_OPEN:-open} i=0
  "$opener" "$HOME/Applications/Orbitty Nightly.app" >>"$RUN_DIR/relaunch.log" 2>&1 || finish_blocked "open Nightly failed"
  for (( i = 0; i < 60; i++ )); do
    if "$CTL" status >>"$RUN_DIR/relaunch.log" 2>&1; then
      flush_notes
      return 0
    fi
    sleep 1
  done
  finish_blocked "Nightly did not come back within 60s"
}

ctl_ok() {
  if [[ $CTL != /* ]]; then
    finish_blocked "ctl: not an absolute path ($CTL)"
  fi
  if [[ $CTL == $HOME/.local/bin/orbittyctl ]]; then
    finish_blocked "ctl: stale ~/.local/bin/orbittyctl (talks to Stable)"
  fi
  if [[ ! -x $CTL || -d $CTL ]]; then
    finish_blocked "ctl: not executable ($CTL)"
  fi
}

# --- preflight (fixed checks, then runner) ---
check_disk
check_rig_ok
check_heavy
ctl_ok
seal_ok
worktree_ok
take_lock
load_runner
validate_prog "$COOK" cook
validate_prog "$VERIFY" verify
snapshot
STAGE=cooking
write_state
"$CTL" kanban move --id "$CARD" --status doing >>"$RUN_DIR/ctl.out" 2>&1 \
  || finish_blocked "could not move the card to doing"
note "GO run $RUN_ID: cooking ($RUNNER_ID)"

# --- cook / verify / fix rounds ---
run_cook
apply_tripwire
run_verify
verify_line=$RESULT_LINE
apply_tripwire

# PASS and PASS-with-notes continue. FAIL fixes, at most twice. Else block.
handle_verify() {
  local line=$1
  if [[ $line == *"RIG BLOCKER"* ]]; then
    finish_blocked "verify: $line"
  fi
  if [[ $line == "RESULT: PASS"* ]]; then
    return 0
  fi
  if [[ $line == "RESULT: FAIL"* ]]; then
    return 1
  fi
  finish_blocked "verify: ${line:-no RESULT line}"
}

if ! handle_verify "$verify_line"; then
  while (( ROUND < 2 )); do
    ROUND=$((ROUND + 1))
    local_report=$RUN_DIR/verify-$((ROUND - 1)).log
    note "GO run $RUN_ID: fixing (round $ROUND)"
    run_cook "$local_report"
    apply_tripwire
    run_verify
    verify_line=$RESULT_LINE
    apply_tripwire
    if handle_verify "$verify_line"; then
      break
    fi
    if (( ROUND >= 2 )); then
      finish_blocked "verify still FAIL after 2 fix rounds"
    fi
  done
fi

REPORT=$(report_from_line "$verify_line")
if [[ -z $REPORT || ! -f $REPORT ]]; then
  finish_blocked "verify report missing"
fi
log_set report "$REPORT"

# --- audit ---
STAGE=auditing
write_state
note "GO run $RUN_ID: auditing"
audit=${GO_RUN_AUDIT:-verify-audit.sh}
"$audit" "$STARTED" > "$RUN_DIR/audit.log" 2>&1 || finish_blocked "verify-audit failed"
log_set audit "$RUN_DIR/audit.log"

# --- re-check, install, match ---
check_disk
check_rig_ok
seal_ok
STAGE=installing
write_state
note "GO run $RUN_ID: installing Nightly"
if [[ -n ${GO_RUN_INSTALL:-} ]]; then
  "$GO_RUN_INSTALL" > "$RUN_DIR/install.log" 2>&1 || finish_blocked "install-nightly failed"
else
  (cd "$WT" && zsh ./scripts/install-nightly.sh) > "$RUN_DIR/install.log" 2>&1 || finish_blocked "install-nightly failed"
fi
log_set install "$RUN_DIR/install.log"
match=${GO_RUN_MATCH:-$WT/scripts/check-install-match.sh}
"$match" > "$RUN_DIR/match.log" 2>&1 || finish_blocked "install match failed"
installed=$(installed_commit)
installed=${installed//$'
'/}
if [[ $installed != $TIP ]]; then
  finish_blocked "installed commit ${installed:-missing} != tip $TIP"
fi

# --- relaunch + dogfood ---
relaunch
tip8=${TIP:0:8}
proj=$(card_project) || finish_blocked "card project unreadable"
why=$(dogfood_why "$REPORT")
need_title="Dogfood $GO_ID ${tip8}?"
need_out=$RUN_DIR/needed.out
if ! "$CTL" needed open --title "$need_title" --project "$proj" --why "$why" >"$need_out" 2>>"$RUN_DIR/ctl.out"; then
  finish_blocked "needed open failed: $(cat "$need_out" 2>/dev/null)"
fi
NEED_ID=$(python3 -c 'import re,sys; t=open(sys.argv[1], errors="replace").read(); m=re.search(r"\bn-[A-Za-z0-9._-]+", t); print(m.group(0) if m else "")' "$need_out")
if [[ -z $NEED_ID ]]; then
  finish_blocked "needed open failed: $(cat "$need_out" 2>/dev/null)"
fi
file_url=$(file_url_for "$REPORT")
if ! "$CTL" kanban link --id "$NEED_ID" --add "$file_url" --label "Verify report" >>"$RUN_DIR/ctl.out" 2>&1; then
  finish_blocked "kanban link failed: $(tail -n 5 "$RUN_DIR/ctl.out")"
fi
note "GO run $RUN_ID: Needed $NEED_ID"
RESULT_LINE="DOGFOOD ready $tip8 on Nightly: see $REPORT ## DOGFOOD (Matt)"
STAGE=done
FINISHED=1
write_state
note "GO run $RUN_ID: $RESULT_LINE"
flush_notes
release_lock
webhook
print -r -- "$RESULT_LINE"
exit 0
