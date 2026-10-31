#!/bin/zsh
# Stub-only conductor tests. Never touches the live app, ~/bin, or real config.
set -euo pipefail

ROOT=${0:A:h:h}
CONDUCTOR=$ROOT/scripts/go-run.sh
MATCH=$ROOT/scripts/check-install-match.sh
fail=0

bad() {
  print -r -- "FAIL $*" >&2
  fail=1
}
ok() { print -r -- "ok $*"; }

zsh -n "$CONDUCTOR"
zsh -n "$MATCH"
zsh -n "$ROOT/scripts/install-nightly.sh"
python3 - "$ROOT/scripts/install-nightly.sh" <<'PY'
import sys
lines = open(sys.argv[1]).read().splitlines()
last = max(i for i, l in enumerate(lines) if 'sign_macos_helpers_then_app "$DEST"' in l)
commit = next(i for i, l in enumerate(lines) if "OrbittyGitCommit" in l)
dirty = next(i for i, l in enumerate(lines) if "OrbittyGitDirty" in l)
assert commit < last and dirty < last, (commit, dirty, last)
PY

forbidden() {
  local hit
  hit=$(grep -n -E 'git push|git merge|go seal|go\.seal|install-stable|pkill|kill |renice' "$CONDUCTOR" || true)
  [[ -z $hit ]] || bad "forbidden in go-run.sh: $hit"
}

setup() {
  HOME_T=$(mktemp -d)
  export GO_RUN_HOME=$HOME_T
  export HOME=$HOME_T
  WT=$HOME_T/wt
  mkdir -p "$WT" "$HOME_T/bin" "$HOME_T/Applications/Orbitty Nightly.app" "$HOME_T/Orbitty GOs"
  git -C "$WT" init -q -b go/test
  git -C "$WT" config user.email t@example.com
  git -C "$WT" config user.name t
  print -r -- a > "$WT/a"
  git -C "$WT" add a
  git -C "$WT" commit -q -m a
  git -C "$WT" branch main
  print -r -- b >> "$WT/a"
  git -C "$WT" add a
  git -C "$WT" commit -q -m b
  BASE=$(git -C "$WT" rev-parse HEAD)
  GOFILE=$HOME_T/Orbitty\ GOs/GO-TEST.md
  cat > "$GOFILE" <<EOF
---
id: GO-TEST
status: sealed
base: '$BASE'
branch: go/test
worktree: $WT
---
body
EOF
  cat > "$HOME_T/bin/cook-pi.sh" <<'EOF'
#!/bin/zsh
{
  print -r -- "SEAT_COOK_MODEL=${SEAT_COOK_MODEL-<unset>}"
  print -r -- "SEAT_VERIFY_MODEL=${SEAT_VERIFY_MODEL-<unset>}"
  print -r -- "SEAT_VERIFY_TIER=${SEAT_VERIFY_TIER-<unset>}"
  print -r -- "SEAT_PROVIDER_PREFS=${SEAT_PROVIDER_PREFS-<unset>}"
  print -r -- "SEAT_VERIFY_BACKUP=${SEAT_VERIFY_BACKUP-<unset>}"
  print -r -- "SEAT_RESERVE_MODEL=${SEAT_RESERVE_MODEL-<unset>}"
} >> "$HOME/.cook-env"
print -r -- "$*" >> "$HOME/.cook-argv"
mode=ok
[[ -f $HOME/.cook-mode ]] && mode=$(<"$HOME/.cook-mode")
cook_done() {
  git -C "$2" commit --allow-empty -q -m cook
  local tip
  tip=$(git -C "$2" rev-parse --short=8 HEAD)
  print -r -- "DONE: $1 $tip"
  print -r -- "EXIT 0 tip=$tip"
}
case $mode in
  blocked) print -r -- "BLOCKED: $1 nope"; print -r -- "EXIT 0 tip=none"; exit 1 ;;
  rig) print -r -- "RIG BLOCKER: $1 disk"; print -r -- "EXIT 0 tip=none"; exit 1 ;;
  later-blocked)
    print -r -- "DONE: $1 early"
    print -r -- "BLOCKED: $1 later"
    print -r -- "EXIT 0 tip=none"
    ;;
  exit1)
    git -C "$2" commit --allow-empty -q -m cook
    tip=$(git -C "$2" rev-parse --short=8 HEAD)
    print -r -- "DONE: $1 $tip"
    print -r -- "EXIT 1 tip=$tip"
    ;;
  remote)
    git -C "$2" commit --allow-empty -q -m cook
    git -C "$2" update-ref refs/remotes/origin/go/test HEAD
    tip=$(git -C "$2" rev-parse --short=8 HEAD)
    print -r -- "DONE: $1 $tip"
    print -r -- "EXIT 0 tip=$tip"
    ;;
  main)
    git -C "$2" commit --allow-empty -q -m cook
    git -C "$2" branch -f main HEAD
    tip=$(git -C "$2" rev-parse --short=8 HEAD)
    print -r -- "DONE: $1 $tip"
    print -r -- "EXIT 0 tip=$tip"
    ;;
  stable)
    git -C "$2" commit --allow-empty -q -m cook
    mkdir -p "$HOME/Applications/Orbitty.app/Contents/MacOS"
    print -r -- x >> "$HOME/Applications/Orbitty.app/Contents/MacOS/orbitty"
    tip=$(git -C "$2" rev-parse --short=8 HEAD)
    print -r -- "DONE: $1 $tip"
    print -r -- "EXIT 0 tip=$tip"
    ;;
  flip)
    git -C "$2" commit --allow-empty -q -m cook
    print -r -- other > "$HOME/.runner-id"
    tip=$(git -C "$2" rev-parse --short=8 HEAD)
    print -r -- "DONE: $1 $tip"
    print -r -- "EXIT 0 tip=$tip"
    ;;
  *) cook_done "$1" "$2" ;;
esac
EOF
  cat > "$HOME_T/bin/verify-pi.sh" <<'EOF'
#!/bin/zsh
{
  print -r -- "SEAT_COOK_MODEL=${SEAT_COOK_MODEL-<unset>}"
  print -r -- "SEAT_VERIFY_MODEL=${SEAT_VERIFY_MODEL-<unset>}"
  print -r -- "SEAT_VERIFY_TIER=${SEAT_VERIFY_TIER-<unset>}"
  print -r -- "SEAT_PROVIDER_PREFS=${SEAT_PROVIDER_PREFS-<unset>}"
} >> "$HOME/.verify-env"
print -r -- "$*" >> "$HOME/.verify-argv"
mode=pass
[[ -f $HOME/.verify-mode ]] && mode=$(<"$HOME/.verify-mode")
report="$HOME/Orbitty GOs/VERIFY_GO-TEST_stub.md"
mkdir -p "$HOME/Orbitty GOs"
if [[ ! -f $report ]]; then
  print -r -- $'## DOGFOOD (Matt)\n1. Look. Pass/Fail.\n## END\n' > "$report"
fi
case $mode in
  fail)
    print -r -- "RESULT: FAIL"
    print -r -- "EXIT 0"
    exit 1
    ;;
  fail-then-pass)
    if [[ ! -f $HOME/.verify-failed ]]; then
      touch "$HOME/.verify-failed"
      print -r -- "RESULT: FAIL"
      print -r -- "EXIT 0"
      exit 1
    fi
    print -r -- "RESULT: PASS-with-notes stub $report"
    print -r -- "EXIT 0"
    ;;
  pass-then-fail)
    print -r -- "RESULT: PASS-with-notes stub $report"
    print -r -- "RESULT: FAIL"
    print -r -- "EXIT 0"
    ;;
  exit1)
    print -r -- "RESULT: PASS-with-notes stub $report"
    print -r -- "EXIT 1"
    ;;
  noreport)
    print -r -- "RESULT: PASS-with-notes stub $HOME/Orbitty GOs/VERIFY_missing.md"
    print -r -- "EXIT 0"
    ;;
  *)
    print -r -- "RESULT: PASS-with-notes stub $report"
    print -r -- "EXIT 0"
    ;;
esac
EOF
  cat > "$HOME_T/bin/other-cook.sh" <<'EOF'
#!/bin/zsh
print -r -- "$*" >> "$HOME/.cook-argv"
git -C "$2" commit --allow-empty -q -m other
tip=$(git -C "$2" rev-parse --short=8 HEAD)
print -r -- "DONE: $1 $tip"
print -r -- "EXIT 0 tip=$tip"
EOF
  cat > "$HOME_T/bin/other-verify.sh" <<'EOF'
#!/bin/zsh
print -r -- "$*" >> "$HOME/.verify-argv"
report="$HOME/Orbitty GOs/VERIFY_GO-TEST_stub.md"
mkdir -p "$HOME/Orbitty GOs"
[[ -f $report ]] || print -r -- $'## DOGFOOD (Matt)\n1. Look.\n## END\n' > "$report"
print -r -- "RESULT: PASS-with-notes stub $report"
print -r -- "EXIT 0"
EOF
  chmod 755 "$HOME_T/bin/"*.sh
  cat > "$HOME_T/bin/ctl" <<'EOF'
#!/bin/zsh
print -r -- "$*" >> "$HOME/.ctl-log"
if [[ ${1:-} == --json ]]; then
  shift
fi
case ${1:-}:${2:-} in
  go:runner)
    if [[ -f $HOME/.ctl-runner-exit ]]; then
      exit $(<"$HOME/.ctl-runner-exit")
    fi
    file=""
    prev=""
    for a in "$@"; do
      if [[ $prev == --file ]]; then
        file=$a
      fi
      prev=$a
    done
    RUNNER_FILE=$file python3 - <<'PY'
import json, os
home = os.environ["HOME"]
cook = os.path.join(home, "bin/cook-pi.sh")
verify = os.path.join(home, "bin/verify-pi.sh")

def header_runner(path):
    if not path or not os.path.isfile(path):
        return ""
    lines = open(path).read().splitlines()
    seen = 0
    for line in lines:
        if line.strip() == "---":
            seen += 1
            if seen >= 2:
                break
            continue
        if seen != 1 or not line.startswith("runner:"):
            continue
        rest = line.split(":", 1)[1].strip()
        if " #" in rest:
            rest = rest.split(" #", 1)[0].strip()
        if len(rest) >= 2 and rest[0] == rest[-1] and rest[0] in "'\"":
            rest = rest[1:-1]
        return rest
    return ""

file_runner = header_runner(os.environ.get("RUNNER_FILE", ""))
if file_runner == "cook-pi":
    file_runner = "pi-grok"
if file_runner:
    runner = file_runner
else:
    runner = "pi-grok"
    p = os.path.join(home, ".runner-id")
    if os.path.exists(p):
        runner = open(p).read().strip()
perms = {"allow_push": False, "allow_merge_main": False, "allow_stable_install": False, "allow_send": False}
pp = os.path.join(home, ".perms.json")
if os.path.exists(pp):
    perms.update(json.load(open(pp)))
prefs = {"zdr": True, "data_collection": "deny"}
approved = [
    {"id": "pi-grok", "cook": cook, "verify": verify, "model": "grok", "provider": "pi"},
    {
        "id": "or-typical", "cook": cook, "verify": verify,
        "model": "deepseek/deepseek-v4.1-flash", "provider": "openrouter",
        "verify_model": "z-ai/glm-5.3", "verify_tier": "flex",
        "verify_backup": "openai/gpt-6.1-sol", "provider_prefs": prefs,
    },
    {
        "id": "or-special", "cook": cook, "verify": verify,
        "model": "anthropic/claude-sonnet-5.5", "provider": "openrouter",
        "verify_model": "z-ai/glm-5.3", "verify_tier": "flex",
        "verify_backup": "openai/gpt-6.1-sol",
        "reserve_model": "anthropic/claude-opus-5.5", "provider_prefs": prefs,
    },
]
ap = os.path.join(home, ".approved.json")
if os.path.exists(ap):
    approved = json.load(open(ap))
entry = next((a for a in approved if a.get("id") == runner), None)
err = None if entry else f"runner config: {runner} not approved"
body = {
    "runner": runner,
    "cook": (entry or {}).get("cook", ""),
    "verify": (entry or {}).get("verify", ""),
    "model": (entry or {}).get("model"),
    "provider": (entry or {}).get("provider"),
    "verify_model": (entry or {}).get("verify_model"),
    "verify_tier": (entry or {}).get("verify_tier"),
    "verify_backup": (entry or {}).get("verify_backup"),
    "reserve_model": (entry or {}).get("reserve_model"),
    "provider_prefs": (entry or {}).get("provider_prefs"),
    "approved": approved,
    "permissions": perms,
    "developerMode": False,
    "error": err,
}
print(json.dumps(body))
PY
    ;;
  go:status)
    if [[ -f $HOME/.status-json ]]; then
      cat "$HOME/.status-json"
    else
      print -r -- '{"sealed":true,"note":"","id":"GO-TEST","found":true}'
    fi
    ;;
  state:get)
    python3 - <<'PY'
import json, os
home = os.environ["HOME"]
proj = "p-test"
p = os.path.join(home, ".project-id")
if os.path.exists(p):
    proj = open(p).read().strip()
print(json.dumps({"kanbanCards":[{"id":"c-test","projectId":proj,"title":"GO-TEST"}]}))
PY
    ;;
  needed:open)
    if [[ -f $HOME/.needed-fail ]]; then
      print -r -- "needed open refused"
      exit 1
    fi
    print -r -- "needed n-stub — $*"
    ;;
  kanban:link)
    if [[ -f $HOME/.link-fail ]]; then
      print -r -- "link refused"
      exit 1
    fi
    exit 0
    ;;
  *)
    exit 0
    ;;
esac
EOF
  cat > "$HOME_T/bin/audit" <<'EOF'
#!/bin/zsh
print -r -- "$*" >> "$HOME/.audit-log"
[[ -f $HOME/.audit-fail ]] && exit 1
exit 0
EOF
  cat > "$HOME_T/bin/install" <<'EOF'
#!/bin/zsh
print -r -- "install-nightly $*" >> "$HOME/.install-log"
exit 0
EOF
  cat > "$HOME_T/bin/match" <<'EOF'
#!/bin/zsh
print -r -- "match $*" >> "$HOME/.match-log"
[[ -f $HOME/.match-fail ]] && exit 1
exit 0
EOF
  cat > "$HOME_T/bin/opener" <<'EOF'
#!/bin/zsh
print -r -- "$*" >> "$HOME/.open-log"
exit 0
EOF
  chmod 755 "$HOME_T/bin/ctl" "$HOME_T/bin/audit" "$HOME_T/bin/install" "$HOME_T/bin/match" "$HOME_T/bin/opener"
  export GO_RUN_CTL=$HOME_T/bin/ctl
  export GO_RUN_AUDIT=$HOME_T/bin/audit
  export GO_RUN_INSTALL=$HOME_T/bin/install
  export GO_RUN_MATCH=$HOME_T/bin/match
  export GO_RUN_OPEN=$HOME_T/bin/opener
  print -r -- "$HOME_T/Applications/Orbitty Nightly.app/Contents/MacOS/orbitty" > "$HOME_T/.procs"
  CARD=c-test
  RUN=""
}

run() {
  local id=$1
  RUN=$id
  zsh "$CONDUCTOR" "$CARD" "$GOFILE" "$id" > "$HOME_T/.conductor-out" 2> "$HOME_T/.conductor-err" || true
  local dir="$HOME_T/Orbitty GOs/_go-runs/$id"
  if [[ -d $dir ]]; then
    cp "$HOME_T/.conductor-out" "$dir/conductor.log"
  fi
}

state() { print -r -- "$HOME_T/Orbitty GOs/_go-runs/$RUN/state.json"; }

json_get() {
  python3 -c 'import json,sys; d=json.load(open(sys.argv[1]));
import functools
cur=d
for k in sys.argv[2].split("."):
    cur=cur[k]
print(cur)' "$(state)" "$1"
}

assert_stage() {
  local want=$1
  local got
  got=$(json_get stage) || { bad "$2 no state"; return; }
  [[ $got == $want ]] || bad "$2 stage=$got want=$want"
}

assert_no_forbidden_calls() {
  local log=$HOME_T/.ctl-log blob
  [[ -f $log ]] || return 0
  blob=$(<"$log")
  [[ $blob != *'--status done'* ]] || bad "$1 ctl moved to done"
  [[ $blob != *'git push'* && $blob != *'git merge'* ]] || bad "$1 ctl push/merge"
  if [[ -f $HOME_T/.install-log ]]; then
    [[ $(<"$HOME_T/.install-log") != *install-stable* ]] || bad "$1 stable install"
  fi
  [[ $blob != *'companion.send'* && $blob != *' send '* ]] || bad "$1 send"
  local f
  for f in "$HOME_T"/Orbitty\ GOs/_go-runs/$RUN/*(N); do
    if grep -E -q 'git push|git merge|install-stable' "$f" 2>/dev/null; then
      bad "$1 run log has push/merge/stable ($f)"
    fi
  done
}

assert_perms_false() {
  local k
  for k in allow_push allow_merge_main allow_stable_install allow_send; do
    [[ $(json_get "permissions.$k") == False ]] || bad "$1 perm $k not false"
  done
}

# --- cases ---

forbidden

setup
run r-happy
assert_stage done happy
assert_perms_false happy
[[ $(json_get runner) == pi-grok ]] || bad "happy runner"
tip=$(git -C "$WT" rev-parse HEAD)
cook_line=$(grep '^cook ' "$HOME_T/Orbitty GOs/_go-runs/r-happy/argv.log" | head -1)
verify_line=$(grep '^verify ' "$HOME_T/Orbitty GOs/_go-runs/r-happy/argv.log" | head -1)
[[ $cook_line == "cook $HOME_T/bin/cook-pi.sh GO-TEST $WT $GOFILE" ]] || bad "happy cook argv: $cook_line"
[[ $verify_line == "verify $HOME_T/bin/verify-pi.sh GO-TEST $tip $WT $GOFILE" ]] || bad "happy verify argv: $verify_line"
moves=$(grep -c 'kanban move' "$HOME_T/.ctl-log" || true)
[[ $moves == 1 ]] || bad "happy moves=$moves"
grep -q -- '--status doing' "$HOME_T/.ctl-log" || bad "happy no doing move"
grep -q 'cooking (pi-grok)' "$HOME_T/.ctl-log" || bad "happy no cooking note"
grep -q 'DOGFOOD ready' "$HOME_T/.ctl-log" || bad "happy no dogfood note"
[[ -f $HOME_T/.quit-log ]] || bad "happy no quit"
grep -q 'Orbitty Nightly' "$HOME_T/.open-log" || bad "happy no open"
[[ ! -d $HOME_T/Orbitty\ GOs/_go-runs/LOCK ]] || bad "happy lock held"
assert_no_forbidden_calls happy
ok happy

setup
print -r -- flip > "$HOME_T/.cook-mode"
run r-flip
assert_stage done flip-run1
verify1=$(grep '^verify ' "$HOME_T/Orbitty GOs/_go-runs/r-flip/argv.log")
[[ $verify1 == *"bin/verify-pi.sh"* ]] || bad "flip run1 used a new verify: $verify1"
git -C "$WT" reset -q --hard "$BASE"
print -r -- other > "$HOME_T/.runner-id"
python3 - <<PY
import json, os
home = os.environ["HOME"]
json.dump([
  {"id":"pi-grok","cook":home+"/bin/cook-pi.sh","verify":home+"/bin/verify-pi.sh"},
  {"id":"other","cook":home+"/bin/other-cook.sh","verify":home+"/bin/other-verify.sh"},
], open(home+"/.approved.json","w"))
PY
: > "$HOME_T/.ctl-log"
run r-flip2
assert_stage done flip-run2
cook2=$(grep '^cook ' "$HOME_T/Orbitty GOs/_go-runs/r-flip2/argv.log" | head -1)
[[ $cook2 == "cook $HOME_T/bin/other-cook.sh GO-TEST $WT $GOFILE" ]] || bad "flip run2 argv: $cook2"
[[ $(json_get runner) == other ]] || bad "flip run2 runner"
assert_no_forbidden_calls flip
ok runner-switch

setup
print -r -- nope > "$HOME_T/.runner-id"
run r-nope
assert_stage blocked unapproved
grep -q 'runner config:' "$HOME_T/.conductor-out" || bad "unapproved missing runner config"
[[ ! -f $HOME_T/.cook-argv ]] || bad "unapproved spawned cook"
assert_no_forbidden_calls unapproved
ok unapproved

setup
mkdir -p "$HOME_T/bin"
print -r -- '#!/bin/zsh' > "$HOME_T/bin/bad-cook.sh"
chmod 644 "$HOME_T/bin/bad-cook.sh"
python3 - <<PY
import json, os
home = os.environ["HOME"]
json.dump([{"id":"bad","cook":home+"/bin/bad-cook.sh","verify":home+"/bin/verify-pi.sh"}], open(home+"/.approved.json","w"))
PY
print -r -- bad > "$HOME_T/.runner-id"
run r-bad
assert_stage blocked nonexec
grep -q 'runner config:' "$HOME_T/.conductor-out" || bad "nonexec missing runner config"
[[ ! -f $HOME_T/.cook-argv ]] || bad "nonexec spawned cook"
ok nonexec

setup
print -r -- blocked > "$HOME_T/.cook-mode"
run r-cblock
assert_stage blocked cook-blocked
[[ ! -f $HOME_T/.verify-argv ]] || bad "cook-blocked ran verify"
grep -q 'BLOCKED:' "$HOME_T/.conductor-out" || bad "cook-blocked no line"
ok cook-blocked

setup
print -r -- fail-then-pass > "$HOME_T/.verify-mode"
run r-ftp
assert_stage done fail-then-pass
[[ $(json_get round) == 1 ]] || bad "fail-then-pass round $(json_get round)"
[[ $(grep -c '^cook ' "$HOME_T/Orbitty GOs/_go-runs/r-ftp/argv.log") == 2 ]] || bad "fail-then-pass cook count"
[[ $(grep -c '^verify ' "$HOME_T/Orbitty GOs/_go-runs/r-ftp/argv.log") == 2 ]] || bad "fail-then-pass verify count"
grep -q 'verify-0.log' "$HOME_T/Orbitty GOs/_go-runs/r-ftp/argv.log" || bad "fail-then-pass missing extra notes"
assert_no_forbidden_calls fail-then-pass
ok fail-then-pass

setup
print -r -- fail > "$HOME_T/.verify-mode"
run r-fx3
assert_stage blocked fail-x3
[[ $(json_get round) == 2 ]] || bad "fail-x3 round $(json_get round)"
[[ $(grep -c '^cook ' "$HOME_T/Orbitty GOs/_go-runs/r-fx3/argv.log") == 3 ]] || bad "fail-x3 cook count"
[[ $(grep -c '^verify ' "$HOME_T/Orbitty GOs/_go-runs/r-fx3/argv.log") == 3 ]] || bad "fail-x3 verify count"
grep -q '2 fix rounds' "$HOME_T/.conductor-out" || bad "fail-x3 reason"
assert_no_forbidden_calls fail-x3
ok fail-x3

setup
touch "$HOME_T/.audit-fail"
run r-audit
assert_stage blocked audit
[[ ! -f $HOME_T/.install-log ]] || bad "audit ran install"
grep -q 'verify-audit failed' "$HOME_T/.conductor-out" || bad "audit reason"
[[ -s $HOME_T/.audit-log ]] || bad "audit not called"
ok audit

setup
touch "$HOME_T/.match-fail"
run r-match
assert_stage blocked match
[[ -f $HOME_T/.install-log ]] || bad "match skipped install"
[[ ! -f $HOME_T/.quit-log ]] || bad "match quit after mismatch"
grep -q 'install match failed' "$HOME_T/.conductor-out" || bad "match reason"
ok match

trip() {
  local mode=$1 key=$2
  setup
  print -r -- "$mode" > "$HOME_T/.cook-mode"
  if [[ $mode == stable ]]; then
    mkdir -p "$HOME_T/Applications/Orbitty.app/Contents/MacOS"
    print -r -- before > "$HOME_T/Applications/Orbitty.app/Contents/MacOS/orbitty"
  fi
  run "r-$mode"
  assert_stage blocked "trip-$mode"
  grep -q "tripwire:" "$HOME_T/.conductor-out" || bad "trip-$mode reason"
  [[ ! -f $HOME_T/.install-log ]] || bad "trip-$mode installed"
  assert_no_forbidden_calls "trip-$mode"
  ok "trip-$mode"
}
trip remote allow_push
trip main allow_merge_main
trip stable allow_stable_install

perm_ok() {
  local mode=$1 key=$2
  setup
  print -r -- "$mode" > "$HOME_T/.cook-mode"
  python3 -c 'import json,sys; json.dump({sys.argv[1]: True}, open(sys.argv[2],"w"))' "$key" "$HOME_T/.perms.json"
  if [[ $mode == stable ]]; then
    mkdir -p "$HOME_T/Applications/Orbitty.app/Contents/MacOS"
    print -r -- before > "$HOME_T/Applications/Orbitty.app/Contents/MacOS/orbitty"
  fi
  run "r-ok-$mode"
  assert_stage done "perm-$key"
  grep -q "$key" "$HOME_T/.ctl-log" || bad "perm-$key no note"
  assert_no_forbidden_calls "perm-$key"
  ok "perm-$key"
}
perm_ok remote allow_push
perm_ok main allow_merge_main
perm_ok stable allow_stable_install

setup
print -r -- 2 > "$HOME_T/.ctl-runner-exit"
run r-down
assert_stage done unreachable
assert_perms_false unreachable
cook_line=$(grep '^cook ' "$HOME_T/Orbitty GOs/_go-runs/r-down/argv.log" | head -1)
[[ $cook_line == "cook $HOME_T/bin/cook-pi.sh GO-TEST $WT $GOFILE" ]] || bad "unreachable argv: $cook_line"
ok unreachable

setup
mkdir -p "$HOME_T/rgl-proofs/job/logs"
touch "$HOME_T/rgl-proofs/job/logs/RIG_OK"
run r-rig
assert_stage blocked rig
[[ ! -f $HOME_T/.cook-argv ]] || bad "rig spawned cook"
grep -q 'RIG_OK' "$HOME_T/.conductor-out" || bad "rig reason"
ok rig

setup
print -r -- 1000 > "$HOME_T/.df-kib"
run r-disk
assert_stage blocked disk
[[ ! -f $HOME_T/.cook-argv ]] || bad "disk spawned cook"
grep -q 'disk' "$HOME_T/.conductor-out" || bad "disk reason"
ok disk

setup
mkdir -p "$HOME_T/Orbitty GOs/_go-runs/LOCK"
print -r -- other > "$HOME_T/Orbitty GOs/_go-runs/LOCK/card"
run r-lock
assert_stage blocked lock
[[ ! -f $HOME_T/.cook-argv ]] || bad "lock spawned cook"
grep -q 'lock' "$HOME_T/.conductor-out" || bad "lock reason"
[[ -d $HOME_T/Orbitty\ GOs/_go-runs/LOCK ]] || bad "lock removed a lock it did not take"
ok lock

setup
print -r -- '{"sealed":false,"note":"GO-TEST is draft, not sealed; not launching.","found":true}' > "$HOME_T/.status-json"
run r-unsealed
assert_stage blocked unsealed
[[ ! -f $HOME_T/.cook-argv ]] || bad "unsealed spawned cook"
grep -q 'not sealed' "$HOME_T/.conductor-out" || bad "unsealed reason"
ok unsealed

setup
print -r -- "$HOME_T/Applications/Orbitty.app/Contents/MacOS/orbitty" > "$HOME_T/.procs"
run r-stable
assert_stage done stable-running
[[ ! -f $HOME_T/.quit-log ]] || bad "stable quit"
grep -q 'not quitting' "$HOME_T/.ctl-log" || bad "stable no skip note"
assert_no_forbidden_calls stable-running
ok stable-running

setup
run r-logs
assert_stage done logs
[[ -n $(json_get logs.cook) ]] || bad "logs cook empty"
[[ -n $(json_get logs.verify) ]] || bad "logs verify empty"
[[ -n $(json_get logs.audit) ]] || bad "logs audit empty"
[[ -n $(json_get logs.install) ]] || bad "logs install empty"
[[ -n $(json_get logs.report) ]] || bad "logs report empty"
[[ -f $(json_get logs.report) ]] || bad "logs report missing file"
grep -q 'DOGFOOD ready' "$HOME_T/.ctl-log" || bad "logs no dogfood"
report=$(json_get logs.report)
grep -q -- "$report" "$HOME_T/.conductor-out" || bad "dogfood line missing report path"
if grep -E -q '^i=' "$HOME_T/.conductor-out" "$HOME_T/.conductor-err" "$HOME_T/Orbitty GOs/_go-runs/r-logs/conductor.log" 2>/dev/null; then
  bad "i= lines in conductor log"
fi
grep -q 'os.argv' "$CONDUCTOR" && bad "os.argv still in go-run.sh"
ok logs

setup
print -r -- later-blocked > "$HOME_T/.cook-mode"
run r-later-blocked
assert_stage blocked later-blocked
grep -q 'BLOCKED:' "$HOME_T/.conductor-out" || bad "later-blocked no BLOCKED"
[[ ! -f $HOME_T/.verify-argv ]] || bad "later-blocked ran verify"
ok later-blocked

setup
print -r -- exit1 > "$HOME_T/.cook-mode"
run r-cook-exit
assert_stage blocked cook-exit
grep -q 'cook exit 1' "$HOME_T/.conductor-out" || bad "cook-exit reason: $(<$HOME_T/.conductor-out)"
ok cook-exit

setup
print -r -- exit1 > "$HOME_T/.verify-mode"
run r-verify-exit
assert_stage blocked verify-exit
grep -q 'verify exit 1' "$HOME_T/.conductor-out" || bad "verify-exit reason: $(<$HOME_T/.conductor-out)"
[[ ! -f $HOME_T/.install-log ]] || bad "verify-exit installed"
ok verify-exit

setup
print -r -- pass-then-fail > "$HOME_T/.verify-mode"
run r-pass-then-fail
assert_stage blocked pass-then-fail
grep -q '2 fix rounds' "$HOME_T/.conductor-out" || bad "pass-then-fail did not treat last RESULT as FAIL"
ok pass-then-fail

setup
unset GO_RUN_CTL
run r-no-ctl
assert_stage blocked no-ctl
grep -q 'ctl: not an absolute path' "$HOME_T/.conductor-out" || bad "no-ctl reason: $(<$HOME_T/.conductor-out)"
[[ ! -f $HOME_T/.cook-argv ]] || bad "no-ctl spawned cook"
ok no-ctl

setup
export GO_RUN_CTL=orbittyctl
run r-path-ctl
assert_stage blocked path-ctl
grep -q 'ctl: not an absolute path' "$HOME_T/.conductor-out" || bad "path-ctl reason: $(<$HOME_T/.conductor-out)"
ok path-ctl

setup
mkdir -p "$HOME_T/.local/bin"
cp "$HOME_T/bin/ctl" "$HOME_T/.local/bin/orbittyctl"
chmod 755 "$HOME_T/.local/bin/orbittyctl"
export GO_RUN_CTL=$HOME_T/.local/bin/orbittyctl
run r-stale-ctl
assert_stage blocked stale-ctl
grep -q 'stale ~/.local/bin/orbittyctl' "$HOME_T/.conductor-out" || bad "stale-ctl reason: $(<$HOME_T/.conductor-out)"
[[ ! -f $HOME_T/.cook-argv ]] || bad "stale-ctl spawned cook"
ok stale-ctl

setup
print -r -- deadbeefdeadbeefdeadbeefdeadbeefdeadbeef > "$HOME_T/.installed-commit"
run r-sha
assert_stage blocked sha-mismatch
grep -q 'installed commit' "$HOME_T/.conductor-out" || bad "sha reason: $(<$HOME_T/.conductor-out)"
grep -q '!= tip' "$HOME_T/.conductor-out" || bad "sha missing tip compare"
[[ ! -f $HOME_T/.quit-log ]] || bad "sha quit after mismatch"
ok sha-mismatch

setup
run r-needed
assert_stage done needed-open
grep -q 'needed open --title Dogfood GO-TEST' "$HOME_T/.ctl-log" || bad "needed argv: $(<$HOME_T/.ctl-log)"
grep -q -- '--project p-test' "$HOME_T/.ctl-log" || bad "needed project"
grep -q 'kanban link --id n-stub' "$HOME_T/.ctl-log" || bad "link argv"
grep -q -- '--label Verify report' "$HOME_T/.ctl-log" || bad "link label"
grep -q 'file://' "$HOME_T/.ctl-log" || bad "link url"
grep -q 'Needed n-stub' "$HOME_T/.ctl-log" || bad "card note missing needed id"
grep -q 'Look' "$HOME_T/.ctl-log" || bad "why missing dogfood steps"
ok needed-open

setup
touch "$HOME_T/.needed-fail"
run r-needed-fail
assert_stage blocked needed-fail
grep -q 'needed open failed' "$HOME_T/.conductor-out" || bad "needed-fail reason: $(<$HOME_T/.conductor-out)"
ok needed-fail

setup
touch "$HOME_T/.link-fail"
run r-link-fail
assert_stage blocked link-fail
grep -q 'kanban link failed' "$HOME_T/.conductor-out" || bad "link-fail reason: $(<$HOME_T/.conductor-out)"
ok link-fail

setup
print -r -- noreport > "$HOME_T/.verify-mode"
run r-noreport
assert_stage blocked noreport
grep -q 'verify report missing' "$HOME_T/.conductor-out" || bad "noreport reason: $(<$HOME_T/.conductor-out)"
[[ ! -f $HOME_T/.install-log ]] || bad "noreport installed"
ok noreport

set_runner() {
  python3 - "$GOFILE" "$1" <<'PY'
import sys
path, rid = sys.argv[1], sys.argv[2]
text = open(path).read()
needle = "id: GO-TEST\n"
insert = "id: GO-TEST\nrunner: %s\n" % rid
if needle not in text:
    raise SystemExit("no id line")
open(path, "w").write(text.replace(needle, insert, 1))
PY
}

dead_pid() {
  local pid
  sleep 30 &
  pid=$!
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  print -r -- "$pid"
}

setup
set_runner or-typical
run r-or-typical
assert_stage done runner-or-typical
grep -q 'SEAT_COOK_MODEL=deepseek/deepseek-v4.1-flash' "$HOME_T/.cook-env" || bad "or-typical cook model: $(<$HOME_T/.cook-env)"
grep -q 'SEAT_VERIFY_MODEL=z-ai/glm-5.3' "$HOME_T/.verify-env" || bad "or-typical verify model: $(<$HOME_T/.verify-env)"
grep -q 'SEAT_PROVIDER_PREFS=' "$HOME_T/.cook-env" || bad "or-typical prefs missing"
ok runner_or_typical_exports_models

setup
run r-pi-exports
assert_stage done runner-pi-grok-exports
grep -q 'SEAT_COOK_MODEL=<unset>' "$HOME_T/.cook-env" || bad "pi-grok exported cook model: $(<$HOME_T/.cook-env)"
grep -q 'SEAT_VERIFY_MODEL=<unset>' "$HOME_T/.verify-env" || bad "pi-grok exported verify model: $(<$HOME_T/.verify-env)"
grep -q 'SEAT_PROVIDER_PREFS=<unset>' "$HOME_T/.cook-env" || bad "pi-grok exported prefs: $(<$HOME_T/.cook-env)"
ok runner_pi_grok_exports_nothing

setup
set_runner gpt-yolo
run r-yolo
assert_stage blocked runner-unknown
grep -q 'not approved' "$HOME_T/.conductor-out" || bad "unknown reason: $(<$HOME_T/.conductor-out)"
grep -q 'gpt-yolo' "$HOME_T/.conductor-out" || bad "unknown missing value"
[[ ! -f $HOME_T/.cook-argv ]] || bad "unknown spawned cook"
ok runner_unknown_blocks

setup
set_runner or-typical
print -r -- 2 > "$HOME_T/.ctl-runner-exit"
run r-or-down
assert_stage blocked or-ctl-down
grep -q 'not falling back' "$HOME_T/.conductor-out" || bad "or-down reason: $(<$HOME_T/.conductor-out)"
[[ ! -f $HOME_T/.cook-argv ]] || bad "or-down fell back to cook"
ok runner_or_typical_ctl_down_stops

setup
print -r -- "$(dead_pid)" > "$HOME_T/Orbitty GOs/RGL_ACTIVE"
run r-rgl-dead
assert_stage done rgl-dead
ok rgl-dead-pid

setup
print -r -- "$$" > "$HOME_T/Orbitty GOs/RGL_ACTIVE"
run r-rgl-live
assert_stage blocked rgl-live
grep -q 'RGL active' "$HOME_T/.conductor-out" || bad "rgl-live reason: $(<$HOME_T/.conductor-out)"
[[ ! -f $HOME_T/.cook-argv ]] || bad "rgl-live spawned cook"
ok rgl-live-pid

setup
print -r -- '/tmp/rgl-envs/bin/python -B r114_run.py resume' > "$HOME_T/.procs"
run r-r114
assert_stage blocked r114
grep -q 'RGL python' "$HOME_T/.conductor-out" || bad "r114 reason: $(<$HOME_T/.conductor-out)"
[[ ! -f $HOME_T/.cook-argv ]] || bad "r114 spawned cook"
ok heavy-r114

setup
mkdir -p "$HOME_T/rgl-proofs/job/logs"
touch "$HOME_T/rgl-proofs/job/logs/RIG_OK.keep"
run r-rig-tag
assert_stage blocked rig-tag
grep -q 'RIG_OK' "$HOME_T/.conductor-out" || bad "rig-tag reason"
[[ ! -f $HOME_T/.cook-argv ]] || bad "rig-tag spawned cook"
ok rig-tag

setup
mkdir -p "$HOME_T/rgl-proofs/job/logs"
touch "$HOME_T/rgl-proofs/job/logs/RIG_OK.tag.archived"
touch "$HOME_T/rgl-proofs/job/logs/RIG_OK.tag.used_1"
run r-rig-arch
assert_stage done rig-arch
ok rig-archived-ignored

setup
mkdir -p "$HOME_T/Orbitty GOs/_go-runs/LOCK"
print -r -- other > "$HOME_T/Orbitty GOs/_go-runs/LOCK/card"
print -r -- "$(dead_pid)" > "$HOME_T/Orbitty GOs/_go-runs/LOCK/pid"
print -r -- then > "$HOME_T/Orbitty GOs/_go-runs/LOCK/started_at"
run r-stale-lock
assert_stage done stale-lock
grep -q 'stale lock cleared' "$HOME_T/Orbitty GOs/_go-runs/r-stale-lock/lock.log" || bad "stale lock not logged"
ok stale-lock-dead-pid

setup
mkdir -p "$WT/src-tauri"
print -r -- x > "$WT/src-tauri/keep"
git -C "$WT" add src-tauri/keep
git -C "$WT" commit -q -m src
BASE=$(git -C "$WT" rev-parse HEAD)
python3 - "$GOFILE" "$BASE" <<'PY'
import sys
path, base = sys.argv[1], sys.argv[2]
text = open(path).read().splitlines()
out = []
for line in text:
    if line.startswith("base:"):
        out.append("base: '%s'" % base)
    else:
        out.append(line)
open(path, "w").write("\n".join(out) + "\n")
PY
run r-hangar
assert_stage blocked hangar
grep -q 'RIG BLOCKER' "$HOME_T/.conductor-out" || bad "hangar reason: $(<$HOME_T/.conductor-out)"
[[ ! -f $HOME_T/.cook-argv ]] || bad "hangar spawned cook"
ok hangar-guard

if (( fail )); then
  print -r -- "test_go_run.sh: $fail failed" >&2
  exit 1
fi
print -r -- "test_go_run.sh: all cases passed"
exit 0
