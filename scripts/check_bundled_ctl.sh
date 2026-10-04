#!/bin/zsh
# Read-only: Contents/MacOS/orbittyctl must be a real non-empty Mach-O
# whose `--help` exits 0 and prints `usage: orbittyctl`.
# The only write is a temp capture of that stdout, removed on exit.
set -euo pipefail

if (( $# < 1 )) || [[ ! -d $1 ]]; then
  print -r -- "usage: check_bundled_ctl.sh <App.app>" >&2
  exit 2
fi

ctl="$1/Contents/MacOS/orbittyctl"

fail() {
  print -r -- "error: bundled orbittyctl $1: $ctl" >&2
  exit 1
}

if [[ ! -e $ctl && ! -L $ctl ]]; then
  fail "missing"
fi
if [[ -L $ctl ]]; then
  fail "is a symlink"
fi
if [[ ! -f $ctl ]]; then
  fail "not a regular file"
fi
if [[ ! -s $ctl ]]; then
  fail "is empty"
fi
if [[ ! -x $ctl ]]; then
  fail "not executable"
fi

kind="$(file -b "$ctl")" || fail "file(1) failed"
if [[ $kind != *Mach-O* ]]; then
  fail "not Mach-O"
fi

timeout_s="${CHECK_CTL_TIMEOUT:-10}"
help_out="$(mktemp "${TMPDIR:-/tmp}/orbittyctl-help.XXXXXX")"
sleep_pid_file="$(mktemp "${TMPDIR:-/tmp}/orbittyctl-help-sleep.XXXXXX")"
trap 'rm -f -- "$help_out" "$sleep_pid_file"' EXIT

# No GNU timeout. `sleep` is its own process so a fast --help can kill it;
# killing the wrapper shell does not kill sleep (it would be reparented and
# the check would sit until the timer finished).
"$ctl" --help </dev/null >"$help_out" 2>/dev/null &
help_pid=$!
(
  sleep "$timeout_s" &
  print -r -- $! >"$sleep_pid_file"
  # A killed sleep must not then kill a helper that already exited.
  wait || exit 0
  kill -KILL "$help_pid" 2>/dev/null || true
) &
help_watcher=$!
sleep_pid=""
for _ in {1..100}; do
  if [[ -s $sleep_pid_file ]]; then
    sleep_pid=$(<"$sleep_pid_file")
    break
  fi
  if ! kill -0 "$help_pid" 2>/dev/null; then
    break
  fi
  sleep 0.01
done
help_status=0
wait "$help_pid" || help_status=$?
if [[ -n $sleep_pid ]]; then
  kill -KILL "$sleep_pid" 2>/dev/null || true
fi
kill -KILL "$help_watcher" 2>/dev/null || true
wait "$help_watcher" 2>/dev/null || true

if (( help_status > 128 )); then
  fail "timed out after ${timeout_s}s"
fi
if (( help_status != 0 )); then
  fail "--help exited ${help_status}"
fi
if ! grep -F -q 'usage: orbittyctl' "$help_out"; then
  fail "--help stdout missing usage: orbittyctl"
fi

bytes="$(stat -f '%z' "$ctl")"
print -r -- "ok: bundled orbittyctl ${bytes} bytes, Mach-O, --help ok: $ctl"
