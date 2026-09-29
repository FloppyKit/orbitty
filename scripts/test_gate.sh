#!/bin/zsh
# Fixture tests for scripts/gate.sh. No cargo, no npm.
# (a) clean tree → 0
# (b) new /Users/someone/x in src/lib/x.ts → 1 and the line printed
# (c) new .buzz/ literal → 1
# (d) allowlisted line → 0
# plus: allowlist does not hide a different literal, and --compare-errors.
set -euo pipefail

cd "$(dirname "$0")/.."
root=$PWD
real_gate="$root/scripts/gate.sh"

typeset -a tmps
tmps=()
cleanup() {
  local d
  for d in "${tmps[@]}"; do
    rm -rf "$d"
  done
}
trap cleanup EXIT

npass=0
nfail=0

pass() {
  echo "PASS $1"
  npass=$((npass + 1))
}

fail() {
  echo "FAIL $1" >&2
  nfail=$((nfail + 1))
}

make_fixture() {
  local tmp
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/gate-fixture.XXXXXX")
  tmps+=("$tmp")
  mkdir -p "$tmp/scripts" "$tmp/src/lib" "$tmp/src-tauri/src"
  cp "$real_gate" "$tmp/scripts/gate.sh"
  chmod +x "$tmp/scripts/gate.sh"
  printf '%s\n' '# fixture allowlist: path<TAB>substring<TAB># reason' > "$tmp/scripts/gate_allowlist.txt"
  printf '%s\n' '{"svelte_check_errors": 3, "base": "fixture"}' > "$tmp/scripts/gate_baseline.json"
  printf '%s\n' 'export const ok = true;' > "$tmp/src/lib/ok.ts"
  printf '%s\n' 'pub fn ok() {}' > "$tmp/src-tauri/src/ok.rs"
  git -C "$tmp" init -q
  git -C "$tmp" config user.email gate-test@local
  git -C "$tmp" config user.name gate-test
  git -C "$tmp" config commit.gpgsign false
  git -C "$tmp" add -- scripts src
  git -C "$tmp" commit -q -m init
  print -r -- "$tmp"
}

commit_fixture() {
  local tmp="$1" msg="$2"
  git -C "$tmp" add -- scripts src
  git -C "$tmp" commit -q -m "$msg"
}

# --- (a) clean tree ---
tmp=$(make_fixture)
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"literal scan: ok"* ]]; then
  pass "clean_tree_exits_0"
else
  fail "clean_tree_exits_0 (exit $rc)"
  print -r -- "$out" >&2
fi

# --- (b) new /Users/someone/x ---
tmp=$(make_fixture)
printf '%s\n' 'export const p = "/Users/someone/x";' > "$tmp/src/lib/x.ts"
commit_fixture "$tmp" "add users literal"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 1 && "$out" == *'src/lib/x.ts'* && "$out" == *'/Users/someone/x'* ]]; then
  pass "new_users_literal_exits_1"
else
  fail "new_users_literal_exits_1 (exit $rc)"
  print -r -- "$out" >&2
fi

# --- (c) new .buzz/ ---
tmp=$(make_fixture)
printf '%s\n' 'export const p = "see .buzz/secret";' > "$tmp/src/lib/buzz.ts"
commit_fixture "$tmp" "add buzz literal"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 1 && "$out" == *'.buzz/'* ]]; then
  pass "new_buzz_literal_exits_1"
else
  fail "new_buzz_literal_exits_1 (exit $rc)"
  print -r -- "$out" >&2
fi

# --- (d) allowlisted line ---
tmp=$(make_fixture)
printf '%s\n' $'src/lib/kept.ts\t/Users/allowed/keep\t# fixture' >> "$tmp/scripts/gate_allowlist.txt"
printf '%s\n' 'export const p = "/Users/allowed/keep";' > "$tmp/src/lib/kept.ts"
commit_fixture "$tmp" "add allowlisted literal"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"literal scan: ok"* ]]; then
  pass "allowlisted_line_exits_0"
else
  fail "allowlisted_line_exits_0 (exit $rc)"
  print -r -- "$out" >&2
fi

# Allowlist must not hide a different literal in the same file.
tmp=$(make_fixture)
printf '%s\n' $'src/lib/mix.ts\t/Users/allowed/keep\t# fixture' >> "$tmp/scripts/gate_allowlist.txt"
printf '%s\n' 'export const keep = "/Users/allowed/keep";' 'export const bad = "/Users/someone/other";' > "$tmp/src/lib/mix.ts"
commit_fixture "$tmp" "mix allowlisted and new"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 1 && "$out" == *'/Users/someone/other'* && "$out" != *'/Users/allowed/keep'* ]]; then
  pass "allowlist_does_not_hide_other_literal"
else
  fail "allowlist_does_not_hide_other_literal (exit $rc)"
  print -r -- "$out" >&2
fi

# --- baseline compare on a fixture baseline of 3 ---
tmp=$(make_fixture)
rc=0
out=$("$tmp/scripts/gate.sh" --compare-errors 3 2>&1) || rc=$?
if [[ "$rc" == 0 ]]; then
  pass "compare_errors_equal_baseline_exits_0"
else
  fail "compare_errors_equal_baseline_exits_0 (exit $rc)"
  print -r -- "$out" >&2
fi

rc=0
out=$("$tmp/scripts/gate.sh" --compare-errors 2 2>&1) || rc=$?
if [[ "$rc" == 0 ]]; then
  pass "compare_errors_under_baseline_exits_0"
else
  fail "compare_errors_under_baseline_exits_0 (exit $rc)"
  print -r -- "$out" >&2
fi

rc=0
out=$("$tmp/scripts/gate.sh" --compare-errors 4 2>&1) || rc=$?
if [[ "$rc" == 1 && "$out" == *"FAIL"* ]]; then
  pass "compare_errors_above_baseline_exits_1"
else
  fail "compare_errors_above_baseline_exits_1 (exit $rc)"
  print -r -- "$out" >&2
fi

# Real baseline file: N == baseline passes, N == baseline+1 fails.
base_n=$(python3 -c 'import json; print(json.load(open("scripts/gate_baseline.json", encoding="utf-8"))["svelte_check_errors"])')
rc=0
out=$("$real_gate" --compare-errors "$base_n" 2>&1) || rc=$?
if [[ "$rc" == 0 ]]; then
  pass "real_baseline_equal_exits_0"
else
  fail "real_baseline_equal_exits_0 (exit $rc)"
  print -r -- "$out" >&2
fi

rc=0
out=$("$real_gate" --compare-errors $((base_n + 1)) 2>&1) || rc=$?
if [[ "$rc" == 1 ]]; then
  pass "real_baseline_above_exits_1"
else
  fail "real_baseline_above_exits_1 (exit $rc)"
  print -r -- "$out" >&2
fi

echo ""
echo "test_gate.sh: $npass passed, $nfail failed"
if (( nfail > 0 )); then
  exit 1
fi
exit 0
