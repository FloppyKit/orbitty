#!/bin/zsh
# Fixture tests for scripts/check-install-match.sh. Temp dirs only.
# No cargo, no network. Never codesigns the real Nightly app and never writes
# under ~/Applications: HOME is pointed at a temp dir that holds the fake app.
#
# The fixture has two different sets of bytes on purpose:
#   raw release  src-tauri/target/release/<name>            (NOT what install-nightly copies)
#   bundle       src-tauri/target/release/bundle/macos/Orbitty.app/Contents/MacOS/<name>
# The fake installed app is a copy of the bundle. So a default run must pass
# only when the default source is the bundle.
set -euo pipefail

here=${0:A:h}
check="$here/check-install-match.sh"
names=(orbitty orbittyctl orbitty-mcp orbitty-relay)
fail=0

tmp=$(mktemp -d "${TMPDIR:-/tmp}/orbitty-check-match.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT

root=$tmp/root
raw=$root/src-tauri/target/release
bundle_app=$raw/bundle/macos/Orbitty.app
bundle_macos=$bundle_app/Contents/MacOS
fake_home=$tmp/home
inst_app="$fake_home/Applications/Orbitty Nightly.app"
inst_macos=$inst_app/Contents/MacOS

mkdir -p "$raw" "$bundle_macos" "$inst_macos"

pass() { print -r -- "PASS $1"; }
bad() {
  print -r -- "FAIL $1"
  fail=1
}

# Raw release bytes differ from the bundle. Bundle bytes equal the fake install.
seed() {
  local n
  for n in $names; do
    print -r -- "raw-release-$n" > "$raw/$n"
    print -r -- "bundle-$n" > "$bundle_macos/$n"
    print -r -- "bundle-$n" > "$inst_macos/$n"
  done
}

# REPLY = "exit<TAB>stdout<TAB>stderr"
run_check() {
  local cwd=$1
  shift
  local ec=0 out err
  out=$(cd "$cwd" && HOME="$fake_home" "$check" "$@" 2>"$tmp/.err") || ec=$?
  err=$(<"$tmp/.err")
  REPLY="$ec"$'\t'"$out"$'\t'"$err"
}

split() {
  ec=${REPLY%%$'\t'*}
  local rest=${REPLY#*$'\t'}
  out=${rest%%$'\t'*}
  err=${rest#*$'\t'}
}

# expect name want_ec want_stdout want_err_substring... (empty want_err = must be empty)
expect() {
  local name=$1 want_ec=$2 want_out=$3
  shift 3
  local needle missing=0
  split
  if (( ec != want_ec )) || [[ $out != "$want_out" ]]; then
    bad "$name"
    print -r -- "  exit=$ec (want $want_ec) out=$out (want '$want_out') err=$err" >&2
    return
  fi
  for needle in "$@"; do
    if [[ $err != *"$needle"* ]]; then
      bad "$name"
      print -r -- "  err missing '$needle': $err" >&2
      return
    fi
  done
  # No stray names when nothing was asked for.
  if [[ $# -eq 0 && -n $err ]]; then
    bad "$name"
    print -r -- "  unexpected stderr: $err" >&2
    return
  fi
  pass "$name"
}

seed

# 1. The default must compare the bundle install-nightly copies, not raw release.
run_check "$root"
expect default_compares_the_bundle 0 ""

# 2. An explicit bundle argument still passes.
run_check "$root" "$bundle_app" "$inst_app"
expect explicit_bundle_arg_passes 0 ""

# 3. An explicit raw-release argument still fails: those bytes are not installed.
run_check "$root" "$raw" "$inst_app"
expect explicit_raw_release_still_fails 1 "" \
  "mismatch orbitty" "mismatch orbittyctl" "mismatch orbitty-mcp" "mismatch orbitty-relay"

# 4. Default fails when the installed app differs from the bundle.
seed
print -r -- "drifted-orbittyctl" > "$inst_macos/orbittyctl"
run_check "$root"
expect default_fails_when_installed_drifted 1 "" "mismatch orbittyctl"

# 5. A missing name is still a failure on the default path.
seed
rm -f "$inst_macos/orbitty-mcp"
run_check "$root"
expect default_missing_name_fails 1 "" "missing orbitty-mcp"

# 6. The fixture app was only read: our bytes are still there.
seed
if [[ $(<"$inst_macos/orbitty") == "bundle-orbitty" ]]; then
  pass fixture_install_untouched
else
  bad fixture_install_untouched
fi

exit $fail
