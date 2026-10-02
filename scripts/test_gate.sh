#!/bin/zsh
# Fixture tests for scripts/gate.sh. No cargo, no npm.
# (a) clean tree → 0
# (b) new /Users/someone/x in src/lib/x.ts → 1 and the line printed
# (c) new .buzz/ literal → 1
# (d) allowlisted line → 0
# plus: allowlist does not hide a different literal, and --compare-errors.
# N1: a stale TEMP allowlist entry fails; other stale entries only warn.
# N4: /users/, ~/.BUZZ and a bare "/Users" are hits; /usersettings is not.
# Caps: file-wide max=N fails over the cap, passes at the cap, rejects a bad max=.
# Pins: path check, git-grep errors, node_modules, npm compare, empty substring,
# last error count, cargo failure, count regex, no first-match break, /Users at
# end of line, TEMP read from the reason only. Stubs stand in for cargo and npm.
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

# --- N1: stale TEMP entry fails, stale plain entry warns, used TEMP passes ---
tmp=$(make_fixture)
printf '%s\n' $'src/lib/gone.ts\t/Users/old/line\t# TEMP until X lands' >> "$tmp/scripts/gate_allowlist.txt"
commit_fixture "$tmp" "stale temp entry"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 1 && "$out" == *"stale TEMP entry: src/lib/gone.ts"* && "$out" == *"FAIL (1 stale TEMP"* ]]; then
  pass "stale_temp_entry_exits_1"
else
  fail "stale_temp_entry_exits_1 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
printf '%s\n' $'src/lib/gone.ts\t/Users/old/line\t# another branch carries this' >> "$tmp/scripts/gate_allowlist.txt"
commit_fixture "$tmp" "stale plain entry"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"1 allowlist entries match nothing"* && "$out" == *"  unmatched: src/lib/gone.ts	/Users/old/line"* && "$out" == *"literal scan: ok"* ]]; then
  pass "stale_plain_entry_warns_exits_0"
else
  fail "stale_plain_entry_warns_exits_0 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
printf '%s\n' $'src/lib/t.ts\t/Users/temp/keep\t# TEMP until X lands' >> "$tmp/scripts/gate_allowlist.txt"
printf '%s\n' 'export const p = "/Users/temp/keep";' > "$tmp/src/lib/t.ts"
commit_fixture "$tmp" "temp entry in use"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"literal scan: ok (1 allowlisted, 0 new)"* ]]; then
  pass "temp_entry_in_use_exits_0"
else
  fail "temp_entry_in_use_exits_0 (exit $rc)"
  print -r -- "$out" >&2
fi

# --- N4: case and trailing-slash variants ---
for variant in 'export const p = "/users/someone/x";' 'export const p = "~/.BUZZ/secret";' 'export const p = "/Users";' 'export const p = "/USERS/x";'; do
  tmp=$(make_fixture)
  printf '%s\n' "$variant" > "$tmp/src/lib/v.ts"
  commit_fixture "$tmp" "variant"
  rc=0
  out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
  if [[ "$rc" == 1 && "$out" == *"src/lib/v.ts"* ]]; then
    pass "variant_is_a_hit: $variant"
  else
    fail "variant_is_a_hit: $variant (exit $rc)"
    print -r -- "$out" >&2
  fi
done

tmp=$(make_fixture)
printf '%s\n' 'export const p = "/usersettings/panel";' 'export const q = "/api/users_list";' > "$tmp/src/lib/nothit.ts"
commit_fixture "$tmp" "not a users path"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"literal scan: ok (0 allowlisted, 0 new)"* ]]; then
  pass "usersettings_is_not_a_hit"
else
  fail "usersettings_is_not_a_hit (exit $rc)"
  print -r -- "$out" >&2
fi

# A file-wide .buzz entry also covers ~/.BUZZ lines (case-insensitive match).
tmp=$(make_fixture)
printf '%s\n' $'src/lib/guard.ts\t.buzz\t# file-wide guard' >> "$tmp/scripts/gate_allowlist.txt"
printf '%s\n' 'export const a = "~/.buzz/x";' 'export const b = "~/.BUZZ/y";' > "$tmp/src/lib/guard.ts"
commit_fixture "$tmp" "file-wide buzz"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"literal scan: ok (2 allowlisted, 0 new)"* ]]; then
  pass "file_wide_buzz_entry_covers_case_variants"
else
  fail "file_wide_buzz_entry_covers_case_variants (exit $rc)"
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

# --- file-wide max=N caps ---
tmp=$(make_fixture)
printf '%s\n' $'src/lib/wide.ts\t.buzz\t# max=1 file-wide' >> "$tmp/scripts/gate_allowlist.txt"
printf '%s\n' 'export const a = ".buzz/a";' 'export const b = ".buzz/b";' > "$tmp/src/lib/wide.ts"
commit_fixture "$tmp" "two buzz lines over cap"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 1 && "$out" == *"over cap: src/lib/wide.ts	.buzz (2 > max=1)"* && "$out" == *"FAIL (1 over cap)"* ]]; then
  pass "file_wide_entry_over_cap_exits_1"
else
  fail "file_wide_entry_over_cap_exits_1 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
printf '%s\n' $'src/lib/wide.ts\t.buzz\t# max=1 file-wide' >> "$tmp/scripts/gate_allowlist.txt"
printf '%s\n' 'export const a = ".buzz/a";' > "$tmp/src/lib/wide.ts"
commit_fixture "$tmp" "one buzz line at cap"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"literal scan: ok (1 allowlisted, 0 new; caps 1/1)"* && "$out" != *"over cap:"* ]]; then
  pass "file_wide_entry_at_cap_passes"
else
  fail "file_wide_entry_at_cap_passes (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
printf '%s\n' $'src/lib/wide.ts\t.buzz\t# max=abc file-wide' >> "$tmp/scripts/gate_allowlist.txt"
commit_fixture "$tmp" "bad max"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 2 && "$out" == *"bad max= in allowlist entry: src/lib/wide.ts"* ]]; then
  pass "bad_max_exits_2"
else
  fail "bad_max_exits_2 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
printf '%s\n' $'src/lib/wide.ts\t.buzz\t# max=0 file-wide' >> "$tmp/scripts/gate_allowlist.txt"
commit_fixture "$tmp" "zero max"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 2 && "$out" == *"bad max= in allowlist entry: src/lib/wide.ts"* ]]; then
  pass "max_zero_exits_2"
else
  fail "max_zero_exits_2 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
printf '%s\n' $'src/lib/wide.ts\tmax=1\t# reason has no cap' >> "$tmp/scripts/gate_allowlist.txt"
printf '%s\n' 'export const a = "/Users/a max=1";' 'export const b = "/Users/b max=1";' > "$tmp/src/lib/wide.ts"
commit_fixture "$tmp" "max equals only in substring"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"literal scan: ok (2 allowlisted, 0 new)"* && "$out" != *"over cap:"* ]]; then
  pass "max_in_substring_is_not_a_cap"
else
  fail "max_in_substring_is_not_a_cap (exit $rc)"
  print -r -- "$out" >&2
fi

# --- mutation pins (one test each) ---
tmp=$(make_fixture)
printf '%s\n' $'src/lib/other.ts\t/Users/someone/x\t# other file' >> "$tmp/scripts/gate_allowlist.txt"
printf '%s\n' 'export const p = "/Users/someone/x";' > "$tmp/src/lib/x.ts"
commit_fixture "$tmp" "same substring other path"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 1 && "$out" == *"src/lib/x.ts"* && "$out" == *"/Users/someone/x"* ]]; then
  pass "allowlist_path_mismatch_does_not_hide_hit"
else
  fail "allowlist_path_mismatch_does_not_hide_hit (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
real_git=$(command -v git)
mkdir -p "$tmp/bin"
cat > "$tmp/bin/git" << EOF
#!/bin/zsh
if [[ "\${1:-}" == "--no-pager" ]]; then
  echo "forced git grep failure" >&2
  exit 3
fi
exec ${real_git} "\$@"
EOF
chmod +x "$tmp/bin/git"
rc=0
out=$(PATH="$tmp/bin:$PATH" "$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 2 && "$out" == *"git grep failed (exit 3)"* ]]; then
  pass "git_grep_error_exits_2"
else
  fail "git_grep_error_exits_2 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
mkdir -p "$tmp/bin"
printf '%s\n' '#!/bin/zsh' 'exit 0' > "$tmp/bin/cargo"
printf '%s\n' '#!/bin/zsh' 'echo "npm stub"' 'exit 0' > "$tmp/bin/npm"
chmod +x "$tmp/bin/cargo" "$tmp/bin/npm"
rc=0
out=$(PATH="$tmp/bin:$PATH" "$tmp/scripts/gate.sh" 2>&1) || rc=$?
if [[ "$rc" == 2 && "$out" == *"run npm ci"* ]]; then
  pass "missing_node_modules_exits_2"
else
  fail "missing_node_modules_exits_2 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
mkdir -p "$tmp/bin" "$tmp/node_modules"
printf '%s\n' '#!/bin/zsh' 'exit 0' > "$tmp/bin/cargo"
printf '%s\n' '#!/bin/zsh' 'echo "svelte-check found 3 errors"' 'exit 0' > "$tmp/bin/npm"
chmod +x "$tmp/bin/cargo" "$tmp/bin/npm"
rc=0
out=$(PATH="$tmp/bin:$PATH" "$tmp/scripts/gate.sh" 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"npm run check: ok (3 errors, baseline 3)"* ]]; then
  pass "npm_equal_baseline_exits_0"
else
  fail "npm_equal_baseline_exits_0 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
printf '%s\n' $'src/lib/x.ts\t\t# r' >> "$tmp/scripts/gate_allowlist.txt"
commit_fixture "$tmp" "empty substring"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 2 && "$out" == *"allowlist entry missing substring: src/lib/x.ts"* ]]; then
  pass "empty_substring_exits_2"
else
  fail "empty_substring_exits_2 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
mkdir -p "$tmp/bin" "$tmp/node_modules"
printf '%s\n' '#!/bin/zsh' 'exit 0' > "$tmp/bin/cargo"
printf '%s\n' '#!/bin/zsh' 'echo "svelte-check found 9 errors"' 'echo "svelte-check found 3 errors"' 'exit 0' > "$tmp/bin/npm"
chmod +x "$tmp/bin/cargo" "$tmp/bin/npm"
rc=0
out=$(PATH="$tmp/bin:$PATH" "$tmp/scripts/gate.sh" 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"npm run check: ok (3 errors, baseline 3)"* ]]; then
  pass "npm_last_error_count_wins"
else
  fail "npm_last_error_count_wins (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
mkdir -p "$tmp/bin"
printf '%s\n' '#!/bin/zsh' 'echo "cargo stub failed" >&2' 'exit 1' > "$tmp/bin/cargo"
printf '%s\n' '#!/bin/zsh' 'echo "npm stub"' 'exit 0' > "$tmp/bin/npm"
chmod +x "$tmp/bin/cargo" "$tmp/bin/npm"
rc=0
out=$(PATH="$tmp/bin:$PATH" "$tmp/scripts/gate.sh" 2>&1) || rc=$?
if [[ "$rc" == 1 && "$out" == *"cargo test --lib: FAIL"* ]]; then
  pass "cargo_fail_exits_1"
else
  fail "cargo_fail_exits_1 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
rc=0
out=$("$tmp/scripts/gate.sh" --compare-errors abc 2>&1) || rc=$?
if [[ "$rc" == 2 && "$out" == *"bad count 'abc'"* ]]; then
  pass "compare_errors_non_number_exits_2"
else
  fail "compare_errors_non_number_exits_2 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
printf '%s\n' $'src/lib/x.ts\t/Users/shared\t# first match' >> "$tmp/scripts/gate_allowlist.txt"
printf '%s\n' $'src/lib/x.ts\t/Users/shared\t# TEMP later' >> "$tmp/scripts/gate_allowlist.txt"
printf '%s\n' 'export const p = "/Users/shared/place";' > "$tmp/src/lib/x.ts"
commit_fixture "$tmp" "two entries one line"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"literal scan: ok (1 allowlisted, 0 new)"* && "$out" != *"stale TEMP"* ]]; then
  pass "second_matching_temp_entry_stays_used"
else
  fail "second_matching_temp_entry_stays_used (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
printf '%s\n' 'export const p = "/Users' > "$tmp/src/lib/eol.ts"
commit_fixture "$tmp" "bare users at eol"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 1 && "$out" == *"src/lib/eol.ts"* && "$out" == *"/Users"* ]]; then
  pass "bare_users_at_eol_exits_1"
else
  fail "bare_users_at_eol_exits_1 (exit $rc)"
  print -r -- "$out" >&2
fi

tmp=$(make_fixture)
printf '%s\n' $'src/lib/nope.ts\tTEMP_marker\t# plain reason' >> "$tmp/scripts/gate_allowlist.txt"
commit_fixture "$tmp" "temp only in substring"
rc=0
out=$("$tmp/scripts/gate.sh" --quick 2>&1) || rc=$?
if [[ "$rc" == 0 && "$out" == *"unmatched: src/lib/nope.ts	TEMP_marker"* && "$out" != *"stale TEMP"* ]]; then
  pass "temp_only_in_substring_is_not_temp"
else
  fail "temp_only_in_substring_is_not_temp (exit $rc)"
  print -r -- "$out" >&2
fi

echo ""
echo "test_gate.sh: $npass passed, $nfail failed"
if (( nfail > 0 )); then
  exit 1
fi
exit 0
