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
print -r -- "$*" >> "$HOME/.cook-argv"
mode=ok
[[ -f $HOME/.cook-mode ]] && mode=$(<"$HOME/.cook-mode")
case $mode in
  blocked) print -r -- "BLOCKED: $1 nope"; exit 1 ;;
  rig) print -r -- "RIG BLOCKER: $1 disk"; exit 1 ;;
  remote)
    git -C "$2" commit --allow-empty -q -m cook
    git -C "$2" update-ref refs/remotes/origin/go/test HEAD
    print -r -- "DONE: $1 $(git -C "$2" rev-parse --short=8 HEAD)"
    ;;
  main)
    git -C "$2" commit --allow-empty -q -m cook
    git -C "$2" branch -f main HEAD
    print -r -- "DONE: $1 $(git -C "$2" rev-parse --short=8 HEAD)"
    ;;
  stable)
    git -C "$2" commit --allow-empty -q -m cook
    mkdir -p "$HOME/Applications/Orbitty.app/Contents/MacOS"
    print -r -- x >> "$HOME/Applications/Orbitty.app/Contents/MacOS/orbitty"
    print -r -- "DONE: $1 $(git -C "$2" rev-parse --short=8 HEAD)"
    ;;
  flip)
    git -C "$2" commit --allow-empty -q -m cook
    print -r -- other > "$HOME/.runner-id"
    print -r -- "DONE: $1 $(git -C "$2" rev-parse --short=8 HEAD)"
    ;;
  *)
    git -C "$2" commit --allow-empty -q -m cook
    print -r -- "DONE: $1 $(git -C "$2" rev-parse --short=8 HEAD)"
    ;;
esac
EOF
  cat > "$HOME_T/bin/verify-pi.sh" <<'EOF'
#!/bin/zsh
print -r -- "$*" >> "$HOME/.verify-argv"
mode=pass
[[ -f $HOME/.verify-mode ]] && mode=$(<"$HOME/.verify-mode")
case $mode in
  fail)
    print -r -- "RESULT: FAIL"
    exit 1
    ;;
  fail-then-pass)
    if [[ ! -f $HOME/.verify-failed ]]; then
      touch "$HOME/.verify-failed"
      print -r -- "RESULT: FAIL"
      exit 1
    fi
    print -r -- "RESULT: PASS"
    ;;
  *) print -r -- "RESULT: PASS-with-notes" ;;
esac
EOF
  cat > "$HOME_T/bin/other-cook.sh" <<'EOF'
#!/bin/zsh
print -r -- "$*" >> "$HOME/.cook-argv"
git -C "$2" commit --allow-empty -q -m other
print -r -- "DONE: $1 other"
EOF
  cat > "$HOME_T/bin/other-verify.sh" <<'EOF'
#!/bin/zsh
print -r -- "$*" >> "$HOME/.verify-argv"
print -r -- "RESULT: PASS"
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
    python3 - <<'PY'
import json, os
home = os.environ["HOME"]
runner = "pi-grok"
p = os.path.join(home, ".runner-id")
if os.path.exists(p):
    runner = open(p).read().strip()
perms = {"allow_push": False, "allow_merge_main": False, "allow_stable_install": False, "allow_send": False}
pp = os.path.join(home, ".perms.json")
if os.path.exists(pp):
    perms.update(json.load(open(pp)))
approved = [{
    "id": "pi-grok",
    "cook": os.path.join(home, "bin/cook-pi.sh"),
    "verify": os.path.join(home, "bin/verify-pi.sh"),
    "model": "grok",
    "provider": "pi",
}]
ap = os.path.join(home, ".approved.json")
if os.path.exists(ap):
    approved = json.load(open(ap))
entry = next((a for a in approved if a.get("id") == runner), None)
err = None if entry else f'runner "{runner}" is not on the approved list (approved: pi-grok)'
print(json.dumps({
    "runner": runner,
    "cook": (entry or {}).get("cook", ""),
    "verify": (entry or {}).get("verify", ""),
    "model": (entry or {}).get("model"),
    "provider": (entry or {}).get("provider"),
    "approved": approved,
    "permissions": perms,
    "developerMode": False,
    "error": err,
}))
PY
    ;;
  go:status)
    if [[ -f $HOME/.status-json ]]; then
      cat "$HOME/.status-json"
    else
      print -r -- '{"sealed":true,"note":"","id":"GO-TEST","found":true}'
    fi
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

if (( fail )); then
  print -r -- "test_go_run.sh: $fail failed" >&2
  exit 1
fi
print -r -- "test_go_run.sh: all cases passed"
exit 0
