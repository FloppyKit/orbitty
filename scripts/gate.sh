#!/bin/zsh
# GO-ORBITTY-LOCAL-GATE: one local gate both install scripts run.
#   cargo test --lib
#   npm run check vs scripts/gate_baseline.json
#   no new /Users or .buzz literals under src-tauri/src and src
#   (case-insensitive: /users/, ~/.BUZZ and a bare "/Users" count too)
# Exit 0 ok, 1 a check failed, 2 a tool or input is missing.
# Never installs and never uses the network. --quick and --compare-errors write nothing; the full gate's cargo test writes src-tauri/target and npm run check may write .svelte-kit.
# A stale TEMP entry is reported only when there are no new hits; it still exits 1.
# TEMP is case-sensitive and is read from the reason field only (everything after the second tab).
# A reason may contain max=N. That entry may cover at most N matching lines; more exits 1 (over cap).
set -euo pipefail

cd "$(dirname "$0")/.."
unset CARGO_TARGET_DIR

root=$PWD
allowlist="$root/scripts/gate_allowlist.txt"
baseline="$root/scripts/gate_baseline.json"

quick=0
compare=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --quick)
      quick=1
      shift
      ;;
    --compare-errors)
      if [[ $# -lt 2 ]]; then
        echo "error-count compare: --compare-errors needs a count" >&2
        exit 2
      fi
      compare="$2"
      shift 2
      ;;
    *)
      echo "gate: unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

# Literal substring. Quoted needle so *, ?, and [ in source text stay literal.
literal_contains() {
  [[ "${1#*"$2"}" != "$1" ]]
}

baseline_errors() {
  if [[ ! -f "$baseline" ]]; then
    echo "gate: missing $baseline" >&2
    exit 2
  fi
  if ! command -v python3 >/dev/null 2>&1; then
    echo "gate: missing python3" >&2
    exit 2
  fi
  local n
  if ! n=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["svelte_check_errors"])' "$baseline"); then
    echo "gate: cannot parse svelte_check_errors in $baseline" >&2
    exit 2
  fi
  if [[ ! "$n" =~ ^[0-9]+$ ]]; then
    echo "gate: bad svelte_check_errors in $baseline: $n" >&2
    exit 2
  fi
  print -r -- "$n"
}

run_compare() {
  local n="$1" base
  if [[ ! "$n" =~ ^[0-9]+$ ]]; then
    echo "error-count compare: bad count '$n'" >&2
    exit 2
  fi
  base=$(baseline_errors)
  if (( n > base )); then
    echo "error-count compare: FAIL ($n > baseline $base)"
    exit 1
  fi
  echo "error-count compare: ok ($n <= baseline $base)"
  exit 0
}

load_allowlist() {
  typeset -g -a ALLOW_PATH ALLOW_SUB ALLOW_TEMP ALLOW_USED ALLOW_LINE ALLOW_MAX ALLOW_COUNT
  ALLOW_PATH=()
  ALLOW_SUB=()
  ALLOW_TEMP=()
  ALLOW_USED=()
  ALLOW_LINE=()
  ALLOW_MAX=()
  ALLOW_COUNT=()
  if [[ ! -f "$allowlist" ]]; then
    echo "literal scan: missing $allowlist" >&2
    exit 2
  fi
  local line path substr rest max token after
  # Split on tab without collapsing an empty field (IFS would turn
  # path<TAB><TAB>reason into a non-empty substring).
  while IFS= read -r line || [[ -n "${line:-}" ]]; do
    [[ -z "${line:-}" ]] && continue
    path="${line%%$'\t'*}"
    if [[ "$line" != *$'\t'* ]]; then
      substr=""
      rest=""
    else
      after="${line#*$'\t'}"
      substr="${after%%$'\t'*}"
      if [[ "$after" == "$substr" ]]; then
        rest=""
      else
        rest="${after#*$'\t'}"
      fi
    fi
    [[ "$path" == \#* ]] && continue
    if [[ -z "${substr:-}" ]]; then
      echo "literal scan: allowlist entry missing substring: $path" >&2
      exit 2
    fi
    ALLOW_PATH+=("$path")
    ALLOW_SUB+=("$substr")
    # A reason containing TEMP marks an entry that must be removed once
    # its line is gone (a stale TEMP entry fails the scan).
    if literal_contains "${rest:-}" "TEMP"; then
      ALLOW_TEMP+=(1)
    else
      ALLOW_TEMP+=(0)
    fi
    ALLOW_USED+=(0)
    ALLOW_LINE+=("$path	$substr")
    # max=N comes from the reason only, never the substring
    # (orbittyctl.rs's substring contains "(max 8)").
    # No max= means uncapped (0). max=0 or a non-number exits 2.
    max=0
    if [[ "${rest:-}" =~ 'max=([^[:space:]]*)' ]]; then
      token="${match[1]}"
      if [[ ! "$token" =~ '^[0-9]+$' ]] || (( 10#$token == 0 )); then
        echo "literal scan: bad max= in allowlist entry: $path" >&2
        exit 2
      fi
      max="$token"
    fi
    ALLOW_MAX+=("$max")
    ALLOW_COUNT+=(0)
  done < "$allowlist"
}

run_literal_scan() {
  if ! command -v git >/dev/null 2>&1; then
    echo "literal scan: missing git" >&2
    exit 2
  fi
  load_allowlist
  local hits rc=0
  # color.ui=never so allowlist substrings match source text, not ANSI codes.
  # Case-insensitive: /users/, ~/.BUZZ, and "/Users" with no trailing slash.
  # The /users match stops at a word character, so /usersettings is not a hit.
  hits=$(git --no-pager -c color.ui=never grep -n -I -i -E -e '/users([^a-z0-9_]|$)' -e '\.buzz' -- src-tauri/src src) || rc=$?
  if (( rc > 1 )); then
    echo "literal scan: git grep failed (exit $rc)" >&2
    exit 2
  fi

  local -a bad
  bad=()
  local allow_n=0
  local hit path rest content matched i

  if [[ -n "$hits" ]]; then
    while IFS= read -r hit; do
      [[ -z "$hit" ]] && continue
      path=${hit%%:*}
      rest=${hit#*:}
      content=${rest#*:}
      matched=0
      # Every matching entry counts as used (for the stale check below).
      # Substrings compare case-insensitively, like the grep.
      for ((i = 1; i <= ${#ALLOW_PATH}; i++)); do
        if [[ ${ALLOW_PATH[i]} == "$path" ]] && literal_contains "${content:l}" "${ALLOW_SUB[i]:l}"; then
          matched=1
          ALLOW_USED[i]=1
          ALLOW_COUNT[i]=$((ALLOW_COUNT[i] + 1))
        fi
      done
      if (( matched )); then
        allow_n=$((allow_n + 1))
      else
        bad+=("$hit")
      fi
    done <<< "$hits"
  fi

  local -a stale_temp stale_plain
  stale_temp=()
  stale_plain=()
  for ((i = 1; i <= ${#ALLOW_PATH}; i++)); do
    (( ALLOW_USED[i] )) && continue
    if (( ALLOW_TEMP[i] )); then
      stale_temp+=("${ALLOW_LINE[i]}")
    else
      stale_plain+=("${ALLOW_LINE[i]}")
    fi
  done
  # Other entries may be for lines another branch carries; warn only.
  if (( ${#stale_plain} > 0 )); then
    echo "literal scan: note: ${#stale_plain} allowlist entries match nothing in this tree" >&2
    local entry
    for entry in "${stale_plain[@]}"; do
      print -r -- "  unmatched: $entry" >&2
    done
  fi

  local -a over
  over=()
  for ((i = 1; i <= ${#ALLOW_PATH}; i++)); do
    if (( 10#${ALLOW_MAX[i]} > 0 && 10#${ALLOW_COUNT[i]} > 10#${ALLOW_MAX[i]} )); then
      over+=("over cap: ${ALLOW_PATH[i]}"$'\t'"${ALLOW_SUB[i]} (${ALLOW_COUNT[i]} > max=${ALLOW_MAX[i]})")
    fi
  done

  if (( ${#bad} > 0 || ${#over} > 0 )); then
    if (( ${#bad} > 0 )); then
      for hit in "${bad[@]}"; do
        print -r -- "$hit"
      done
    fi
    if (( ${#over} > 0 )); then
      local line
      for line in "${over[@]}"; do
        print -r -- "$line"
      done
    fi
    if (( ${#bad} > 0 && ${#over} > 0 )); then
      echo "literal scan: FAIL (${#bad} new, ${#over} over cap)"
      exit 1
    fi
    if (( ${#over} > 0 )); then
      echo "literal scan: FAIL (${#over} over cap)"
      exit 1
    fi
    echo "literal scan: FAIL (${#bad} new)"
    exit 1
  fi
  if (( ${#stale_temp} > 0 )); then
    for hit in "${stale_temp[@]}"; do
      print -r -- "stale TEMP entry: $hit"
    done
    echo "literal scan: FAIL (${#stale_temp} stale TEMP allowlist entries; remove them)"
    exit 1
  fi
  local caps=""
  for ((i = 1; i <= ${#ALLOW_PATH}; i++)); do
    if (( 10#${ALLOW_MAX[i]} > 0 )); then
      caps+=" ${ALLOW_COUNT[i]}/${ALLOW_MAX[i]}"
    fi
  done
  if [[ -n "$caps" ]]; then
    echo "literal scan: ok ($allow_n allowlisted, 0 new; caps${caps})"
  else
    echo "literal scan: ok ($allow_n allowlisted, 0 new)"
  fi
}

run_cargo() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo test --lib: missing cargo" >&2
    exit 2
  fi
  unset CARGO_TARGET_DIR
  if ! cargo test --manifest-path src-tauri/Cargo.toml --lib; then
    echo "cargo test --lib: FAIL"
    exit 1
  fi
  echo "cargo test --lib: ok"
}

run_npm_check() {
  if [[ ! -d "$root/node_modules" ]]; then
    echo "run npm ci" >&2
    exit 2
  fi
  if ! command -v npm >/dev/null 2>&1; then
    echo "npm run check: missing npm" >&2
    exit 2
  fi
  local out rc=0 n base
  out=$(npm run check 2>&1) || rc=$?
  n=$(print -r -- "$out" | python3 -c 'import re,sys; ms=re.findall(r"svelte-check found (\d+) errors?", sys.stdin.read()); print(ms[-1] if ms else "")') || {
    echo "npm run check: could not parse error count (exit $rc)" >&2
    exit 2
  }
  if [[ -z "$n" ]]; then
    echo "npm run check: could not parse error count (exit $rc)" >&2
    print -r -- "$out" | tail -30 >&2
    exit 2
  fi
  base=$(baseline_errors)
  if (( n > base )); then
    echo "npm run check: FAIL ($n errors > baseline $base)"
    print -r -- "$out" | tail -40 >&2
    exit 1
  fi
  echo "npm run check: ok ($n errors, baseline $base)"
}

if [[ -n "$compare" ]]; then
  run_compare "$compare"
fi

if (( ! quick )); then
  run_cargo
  run_npm_check
fi
run_literal_scan
