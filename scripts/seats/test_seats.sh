#!/bin/zsh
# Stub pi on PATH. Never calls the real pi, OpenRouter, or ~/bin.
set -euo pipefail

ROOT=${0:A:h:h:h}
COOK=$ROOT/scripts/seats/cook-pi.sh
VERIFY=$ROOT/scripts/seats/verify-pi.sh
fail=0

bad() {
  print -r -- "FAIL $*" >&2
  fail=1
}
ok() { print -r -- "ok $*"; }

zsh -n "$COOK"
zsh -n "$VERIFY"

newhome() {
  HOME=$(mktemp -d)
  export HOME
  mkdir -p "$HOME/bin" "$HOME/Orbitty GOs/_logs" "$HOME/wt"
  WT=$HOME/wt
  GOFILE=$HOME/GO.md
  print -r -- "go" > "$GOFILE"
  git -C "$WT" init -q
  git -C "$WT" config user.email t@example.com
  git -C "$WT" config user.name t
  print -r -- a > "$WT/a"
  git -C "$WT" add a
  git -C "$WT" commit -q -m a
  cat > "$HOME/bin/pi" <<'EOF'
#!/bin/zsh
print -r -- "$*" >> "$HOME/.pi-argv"
print -r -- "CALL $*" >> "$HOME/.pi-calls"
mode=done
[[ -f $HOME/.pi-mode ]] && mode=$(<"$HOME/.pi-mode")
case $mode in
  402)
    print -r -- "402 payment required"
    exit 1
    ;;
  glm-fail)
    if [[ $* == *glm-5.3* ]]; then
      print -r -- "402 payment required"
      exit 1
    fi
    print -r -- "RESULT: PASS-with-notes stub"
    exit 0
    ;;
  both-fail)
    print -r -- "402 payment required"
    exit 1
    ;;
  rig)
    print -r -- "RIG BLOCKER: disk"
    exit 0
    ;;
  result)
    print -r -- "RESULT: PASS-with-notes stub"
    exit 0
    ;;
  *)
    print -r -- "DONE: GO-TEST abc"
    exit 0
    ;;
esac
EOF
  cat > "$HOME/bin/hangar-target.sh" <<'EOF'
#!/bin/zsh
print -r -- "$*" >> "$HOME/.hangar-argv"
if [[ -f $HOME/.hangar-fail ]]; then
  exit 1
fi
exit 0
EOF
  chmod 755 "$HOME/bin/pi" "$HOME/bin/hangar-target.sh"
  export PATH="$HOME/bin:$PATH"
  unset SEAT_COOK_MODEL SEAT_VERIFY_MODEL SEAT_VERIFY_BACKUP SEAT_PI_SENDS_PROVIDER_PREFS SEAT_VERIFY_TIER SEAT_PROVIDER_PREFS
}

confirm_zdr() {
  mkdir -p "$HOME/.config/orbitty"
  : > "$HOME/.config/orbitty/openrouter-zdr-confirmed"
}

run_cook() {
  local rc=0
  "$COOK" GO-TEST "$WT" "$GOFILE" >"$HOME/out" 2>"$HOME/err" || rc=$?
  print -r -- "$rc"
}

run_verify() {
  local rc=0
  "$VERIFY" GO-TEST abcdef01 "$WT" "$GOFILE" >"$HOME/out" 2>"$HOME/err" || rc=$?
  print -r -- "$rc"
}

cook_log() { print -r -- "$HOME"/Orbitty\ GOs/_logs/cook-pi_*(N[-1]); }
verify_log() { print -r -- "$HOME"/Orbitty\ GOs/_logs/verify-pi_*(N[-1]); }

assert_exit_pair() {
  local log=$1
  python3 - "$log" <<'PY' || return 1
import sys
lines = open(sys.argv[1], errors="replace").read().splitlines()
if len(lines) < 2:
    sys.exit(1)
verdict, exit_line = lines[-2], lines[-1]
if not verdict.startswith(("DONE:", "RESULT:", "BLOCKED:", "RIG BLOCKER:", "STOP ")):
    sys.exit(2)
bits = exit_line.split()
if len(bits) < 3 or bits[0] != "EXIT" or not bits[1].lstrip("-").isdigit() or not bits[2].startswith("tip="):
    sys.exit(3)
PY
}

newhome
rc=$(run_cook)
[[ $rc == 0 ]] || bad "no-model rc=$rc"
[[ -f $HOME/.pi-argv ]] || bad "no-model pi not called"
if grep -q -- '--model' "$HOME/.pi-argv"; then
  bad "no-model argv has --model"
fi
ok seat_no_model_no_flag

newhome
export SEAT_COOK_MODEL=deepseek/deepseek-v4.1-flash
rc=$(run_cook)
[[ $rc == 3 ]] || bad "zdr rc=$rc out=$(<$HOME/out)"
log=$(cook_log)
grep -q 'BLOCKED:' "$log" || bad "zdr missing BLOCKED: $(<$log)"
grep -q 'ZDR' "$log" || bad "zdr missing ZDR"
[[ ! -f $HOME/.pi-argv ]] || bad "zdr called pi"
ok seat_model_needs_zdr

newhome
confirm_zdr
export SEAT_COOK_MODEL=deepseek/deepseek-v4.1-flash
print -r -- 402 > "$HOME/.pi-mode"
rc=$(run_cook)
[[ $rc == 3 ]] || bad "route rc=$rc"
[[ $(grep -c '^CALL ' "$HOME/.pi-calls") == 1 ]] || bad "route reran: $(<$HOME/.pi-calls)"
log=$(cook_log)
grep -q '^STOP ' "$log" || bad "route missing STOP: $(<$log)"
if grep -q 'grok' "$HOME/.pi-argv"; then
  bad "route called grok"
fi
ok seat_route_error_stops

newhome
confirm_zdr
export SEAT_VERIFY_MODEL=z-ai/glm-5.3
export SEAT_VERIFY_BACKUP=openai/gpt-6.1-sol
print -r -- glm-fail > "$HOME/.pi-mode"
rc=$(run_verify)
[[ $rc == 0 ]] || bad "sol rc=$rc out=$(<$HOME/out) err=$(<$HOME/err)"
[[ $(grep -c 'glm-5.3' "$HOME/.pi-calls") == 1 ]] || bad "glm calls: $(<$HOME/.pi-calls)"
[[ $(grep -c 'gpt-6.1-sol' "$HOME/.pi-calls") == 1 ]] || bad "sol calls: $(<$HOME/.pi-calls)"
log=$(verify_log)
grep -q 'verify_model_used: openai/gpt-6.1-sol' "$log" || bad "missing verify_model_used: $(<$log)"
grep -q 'fallback_reason:' "$log" || bad "missing fallback_reason: $(<$log)"
ok verify_glm_fail_uses_sol_logged

newhome
confirm_zdr
export SEAT_VERIFY_MODEL=z-ai/glm-5.3
export SEAT_VERIFY_BACKUP=openai/gpt-6.1-sol
print -r -- both-fail > "$HOME/.pi-mode"
rc=$(run_verify)
[[ $rc == 3 ]] || bad "both rc=$rc"
[[ $(grep -c '^CALL ' "$HOME/.pi-calls") == 2 ]] || bad "both call count: $(<$HOME/.pi-calls)"
log=$(verify_log)
grep -q '^STOP ' "$log" || bad "both missing STOP: $(<$log)"
if grep -q 'grok' "$HOME/.pi-calls"; then
  bad "both called grok"
fi
ok verify_both_fail_stops

newhome
print -r -- rig > "$HOME/.pi-mode"
rc=$(run_cook)
[[ $rc == 3 ]] || bad "rig rc=$rc"
log=$(cook_log)
grep -q '^RIG BLOCKER:' "$log" || bad "rig line: $(<$log)"
ok seat_rig_blocker_exits_nonzero

newhome
print -r -- result > "$HOME/.pi-mode"
rc=$(run_verify)
[[ $rc == 0 ]] || bad "exit-line verify rc=$rc"
assert_exit_pair "$(verify_log)" || bad "verify exit pair: $(<$(verify_log))"
print -r -- done > "$HOME/.pi-mode"
rc=$(run_cook)
[[ $rc == 0 ]] || bad "exit-line cook rc=$rc"
assert_exit_pair "$(cook_log)" || bad "cook exit pair: $(<$(cook_log))"
ok seat_exit_line_kept

if (( fail )); then
  print -r -- "test_seats.sh: $fail failed" >&2
  exit 1
fi
print -r -- "test_seats.sh: all cases passed"
exit 0
