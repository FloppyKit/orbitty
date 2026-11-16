#!/bin/zsh
# Compare the installed app to the bundle install-nightly.sh copies, with
# signatures removed. Default source is that bundle, the SRC ditto'd into
# Nightly; the raw cargo outputs in target/release are not the install source.
# An explicit first argument still wins (a fixture dir, an .app, or raw release).
# Copies first. Never strips the build tree or the installed bundle.
# Usage: check-install-match.sh [build_dir | build.app] [installed.app | bin_dir]
set -euo pipefail

names=(orbitty orbittyctl orbitty-mcp orbitty-relay)
build=${1:-src-tauri/target/release/bundle/macos/Orbitty.app}
dest=${2:-$HOME/Applications/Orbitty Nightly.app}

bin_dir() {
  local d=$1
  if [[ -d $d/Contents/MacOS ]]; then
    print -r -- "$d/Contents/MacOS"
  else
    print -r -- "$d"
  fi
}

src=$(bin_dir "$build")
dst=$(bin_dir "$dest")
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail=0
for n in $names; do
  a="$src/$n"
  b="$dst/$n"
  if [[ ! -f $a || ! -f $b ]]; then
    print -r -- "missing $n" >&2
    fail=1
    continue
  fi
  cp "$a" "$tmp/build-$n"
  cp "$b" "$tmp/inst-$n"
  codesign --remove-signature "$tmp/build-$n" 2>/dev/null || true
  codesign --remove-signature "$tmp/inst-$n" 2>/dev/null || true
  if ! cmp -s "$tmp/build-$n" "$tmp/inst-$n"; then
    print -r -- "mismatch $n" >&2
    fail=1
  fi
done
exit $fail
