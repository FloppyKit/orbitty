#!/bin/zsh
# Fixture tests for scripts/check_bundled_ctl.sh. Temp dirs only.
# No cargo, no network. cc is the Xcode CLT compiler build.rs already needs.
set -euo pipefail

here=${0:A:h}
check="$here/check_bundled_ctl.sh"
fail=0
tmp=$(mktemp -d "${TMPDIR:-/tmp}/orbitty-check-ctl.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT

pass() { print -r -- "PASS $1"; }
bad() { print -r -- "FAIL $1"; fail=1; }

cc -o "$tmp/good" -x c - <<'EOF'
#include <stdio.h>
int main(void) {
    fputs("usage: orbittyctl [--json]\n", stdout);
    return 0;
}
EOF

cc -o "$tmp/hang" -x c - <<'EOF'
#include <unistd.h>
int main(void) {
    sleep(30);
    return 0;
}
EOF

macos_of() {
  local name=$1
  local d="$tmp/$name.app/Contents/MacOS"
  mkdir -p "$d"
  REPLY=$d
}

# Run the check. REPLY is "exit<TAB>stdout". Stderr is left in $tmp/$name.err.
run_check() {
  local name=$1 app=$2
  local out err ec
  err="$tmp/$name.err"
  ec=0
  out=$("$check" "$app" 2>"$err") || ec=$?
  REPLY="${ec}"$'\t'"${out}"
}

expect_ok() {
  local name=$1 app=$2 ec out
  run_check "$name" "$app"
  ec=${REPLY%%$'\t'*}
  out=${REPLY#*$'\t'}
  if (( ec == 0 )) && [[ $out == ok:\ bundled\ orbittyctl\ <->\ bytes,\ Mach-O,\ --help\ ok:\ * ]]; then
    pass "$name"
  else
    bad "$name"
    print -r -- "  exit=$ec out=$out" >&2
    [[ -f $tmp/$name.err ]] && cat "$tmp/$name.err" >&2
  fi
}

expect_refuse() {
  local name=$1 app=$2 ec out err
  run_check "$name" "$app"
  ec=${REPLY%%$'\t'*}
  out=${REPLY#*$'\t'}
  err=""
  [[ -f $tmp/$name.err ]] && err=$(<"$tmp/$name.err")
  if (( ec == 1 )) && [[ -z $out ]] && [[ $err == error:\ bundled\ orbittyctl\ *:\ * ]]; then
    pass "$name"
  else
    bad "$name"
    print -r -- "  exit=$ec out=$out err=$err" >&2
  fi
}

macos_of ok_real_macho_help
cp "$tmp/good" "$REPLY/orbittyctl"
chmod 755 "$REPLY/orbittyctl"
expect_ok ok_real_macho_help "$tmp/ok_real_macho_help.app"

macos_of refuses_missing
expect_refuse refuses_missing "$tmp/refuses_missing.app"

macos_of refuses_symlink
ln -s "$tmp/good" "$REPLY/orbittyctl"
expect_refuse refuses_symlink "$tmp/refuses_symlink.app"

macos_of refuses_empty
: > "$REPLY/orbittyctl"
chmod 755 "$REPLY/orbittyctl"
expect_refuse refuses_empty "$tmp/refuses_empty.app"

macos_of refuses_not_executable
cp "$tmp/good" "$REPLY/orbittyctl"
chmod 644 "$REPLY/orbittyctl"
expect_refuse refuses_not_executable "$tmp/refuses_not_executable.app"

macos_of refuses_non_macho_script
cat > "$REPLY/orbittyctl" <<'EOF'
#!/bin/sh
echo "usage: orbittyctl"
exit 0
EOF
chmod 755 "$REPLY/orbittyctl"
expect_refuse refuses_non_macho_script "$tmp/refuses_non_macho_script.app"

macos_of refuses_help_nonzero
cp /usr/bin/false "$REPLY/orbittyctl"
chmod 755 "$REPLY/orbittyctl"
expect_refuse refuses_help_nonzero "$tmp/refuses_help_nonzero.app"

macos_of refuses_help_wrong_output
cp /usr/bin/true "$REPLY/orbittyctl"
chmod 755 "$REPLY/orbittyctl"
expect_refuse refuses_help_wrong_output "$tmp/refuses_help_wrong_output.app"

macos_of refuses_help_hang
cp "$tmp/hang" "$REPLY/orbittyctl"
chmod 755 "$REPLY/orbittyctl"
hang_start=$SECONDS
hang_ec=0
hang_out=$(CHECK_CTL_TIMEOUT=1 "$check" "$tmp/refuses_help_hang.app" 2>"$tmp/refuses_help_hang.err") || hang_ec=$?
hang_elapsed=$(( SECONDS - hang_start ))
hang_err=""
[[ -f $tmp/refuses_help_hang.err ]] && hang_err=$(<"$tmp/refuses_help_hang.err")
if (( hang_ec == 1 )) && [[ -z $hang_out ]] && [[ $hang_err == error:\ bundled\ orbittyctl\ *:\ * ]]; then
  pass refuses_help_hang
else
  bad refuses_help_hang
  print -r -- "  exit=$hang_ec out=$hang_out err=$hang_err" >&2
fi
if (( hang_elapsed >= 10 )); then
  bad "refuses_help_hang_finished_under_10s"
  print -r -- "  elapsed=${hang_elapsed}s" >&2
else
  pass "refuses_help_hang_finished_under_10s"
fi

ec=0
usage_out=$("$check" 2>"$tmp/usage.err") || ec=$?
if (( ec == 2 )); then
  pass usage_error_no_arg
else
  bad usage_error_no_arg
  print -r -- "  exit=$ec out=$usage_out" >&2
fi

exit $fail
