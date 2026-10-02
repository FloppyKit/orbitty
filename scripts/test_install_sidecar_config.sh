#!/bin/zsh
# Prove the marked sidecar-config blocks without running the install scripts.
# Temp HOME dirs only. Exit 0 when every case passes.
set -euo pipefail
umask 022

here=${0:A:h}
nightly=$here/install-nightly.sh
kitchen=$here/install-kitchen.sh

typeset -a temps
cleanup() {
  if (( ${#temps} )); then
    rm -rf -- "${temps[@]}"
  fi
}
trap cleanup EXIT

mk() {
  local d
  d=$(mktemp -d)
  temps+=("$d")
  REPLY=$d
}

# Body between marker lines (markers themselves are not executed).
extract_between() {
  local file=$1 begin=$2 end=$3
  awk -v begin="$begin" -v end="$end" '
    index($0, begin) { grab = 1; next }
    index($0, end) { if (grab) { grab = 0; next } }
    grab { print }
  ' "$file"
}

fail=0
pass() { print -r -- "PASS $1"; }
bad() { print -r -- "FAIL $1"; fail=1; }

run_block() {
  local home=$1 block=$2
  local weights=${3-}
  print -r -- "$block" > "$home/block.zsh"
  if [[ -n $weights ]]; then
    HOME=$home MOUTH_WEIGHTS="$weights" zsh "$home/block.zsh"
  else
    HOME=$home zsh "$home/block.zsh"
  fi
}

nightly_block=$(extract_between "$nightly" \
  "# BEGIN mouth-sidecar-config (GO-ORBITTY-DEMATT-RUNTIME-FIX-R2)" \
  "# END mouth-sidecar-config")
kitchen_block=$(extract_between "$kitchen" \
  "# BEGIN mouth-sidecar-config-kitchen" \
  "# END mouth-sidecar-config-kitchen")

if [[ -n ${nightly_block//[[:space:]]/} ]]; then
  pass "nightly-4-block-nonempty"
else
  bad "nightly-4-block-nonempty"
fi

if [[ -n ${kitchen_block//[[:space:]]/} ]]; then
  pass "kitchen-block-nonempty"
else
  bad "kitchen-block-nonempty"
fi

# 1. plist exists, config missing → write url, valid JSON, mode 600.
mk
home=$REPLY
mkdir -p "$home/Library/LaunchAgents"
: > "$home/Library/LaunchAgents/space.orbitty.mouth.plist"
run_block "$home" "$nightly_block"
cfg=$home/.config/orbitty/mouth-sidecar.json
mode=$(stat -f %Lp "$cfg" 2>/dev/null || print missing)
got=""
if [[ -f $cfg ]]; then
  got=$(<"$cfg")
fi
if [[ $got == '{"url":"http://127.0.0.1:13121"}' && $mode == 600 ]] \
  && python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); raise SystemExit(0 if d=={"url":"http://127.0.0.1:13121"} else 1)' "$cfg"; then
  pass "nightly-1-plist-missing-writes-600"
else
  bad "nightly-1-plist-missing-writes-600"
fi

# 2. plist exists, config exists → content and mode stay.
mk
home=$REPLY
mkdir -p "$home/Library/LaunchAgents" "$home/.config/orbitty"
: > "$home/Library/LaunchAgents/space.orbitty.mouth.plist"
print -r -- '{"url":"http://127.0.0.1:9999"}' > "$home/.config/orbitty/mouth-sidecar.json"
chmod 644 "$home/.config/orbitty/mouth-sidecar.json"
before=$(<"$home/.config/orbitty/mouth-sidecar.json")
before_mode=$(stat -f %Lp "$home/.config/orbitty/mouth-sidecar.json")
run_block "$home" "$nightly_block"
after=$(<"$home/.config/orbitty/mouth-sidecar.json")
after_mode=$(stat -f %Lp "$home/.config/orbitty/mouth-sidecar.json")
if [[ $before == '{"url":"http://127.0.0.1:9999"}' && $before == $after && $before_mode == 644 && $after_mode == $before_mode ]]; then
  pass "nightly-2-existing-untouched"
else
  bad "nightly-2-existing-untouched"
fi

# 3. no plist → no config file and no orbitty config dir.
mk
home=$REPLY
run_block "$home" "$nightly_block"
if [[ ! -e $home/.config/orbitty/mouth-sidecar.json && ! -d $home/.config/orbitty ]]; then
  pass "nightly-3-no-plist-no-write"
else
  bad "nightly-3-no-plist-no-write"
fi

kitchen_one() {
  local name=$1 weights=$2 home cfg mode
  if [[ -z ${kitchen_block//[[:space:]]/} ]]; then
    bad "$name"
    return
  fi
  mk
  home=$REPLY
  run_block "$home" "$kitchen_block" "$weights"
  cfg=$home/.config/orbitty/mouth-sidecar.json
  if [[ ! -f $cfg ]]; then
    bad "$name"
    return
  fi
  mode=$(stat -f %Lp "$cfg")
  if [[ $mode == 600 ]] && python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); raise SystemExit(0 if d.get("url")=="http://127.0.0.1:13121" and d.get("weights")==sys.argv[2] else 1)' "$cfg" "$weights"; then
    pass "$name"
  else
    bad "$name"
  fi
}

kitchen_one "kitchen-quote" 'models/say"quote'
kitchen_one "kitchen-backslash" 'models/back\slash'

mk
home=$REPLY
run_block "$home" "$kitchen_block" "models/normal"
cfg=$home/.config/orbitty/mouth-sidecar.json
mode=$(stat -f %Lp "$cfg" 2>/dev/null || print missing)
if [[ -f $cfg && $mode == 600 ]] && python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); raise SystemExit(0 if d.get("url")=="http://127.0.0.1:13121" and d.get("weights")=="models/normal" else 1)' "$cfg"; then
  pass "cfg_written_600_valid_json"
else
  bad "cfg_written_600_valid_json"
fi

dangling_case() {
  local name=$1 block=$2 weights=${3-}
  mk
  local home=$REPLY
  local outside
  mk
  outside=$REPLY
  mkdir -p "$home/.config/orbitty"
  if [[ $name == nightly_* ]]; then
    mkdir -p "$home/Library/LaunchAgents"
    : > "$home/Library/LaunchAgents/space.orbitty.mouth.plist"
  fi
  ln -s "$outside/victim.json" "$home/.config/orbitty/mouth-sidecar.json"
  local link_before
  link_before=$(readlink "$home/.config/orbitty/mouth-sidecar.json")
  if [[ -n $weights ]]; then
    run_block "$home" "$block" "$weights"
  else
    run_block "$home" "$block"
  fi
  local cfg=$home/.config/orbitty/mouth-sidecar.json
  if [[ -L $cfg && ! -e $cfg && $(readlink "$cfg") == "$link_before" \
        && ! -e $outside/victim.json ]]; then
    pass "$name"
  else
    bad "$name"
  fi
}

dangling_case "nightly_cfg_dangling_symlink_no_write" "$nightly_block"
dangling_case "kitchen_cfg_dangling_symlink_no_write" "$kitchen_block" "models/normal"

# PATH without python3: no config file, no temp left, block exits 0.
mk
home=$REPLY
bindir=$home/bin
mkdir -p "$bindir"
for c in mkdir mktemp chmod mv rm; do
  ln -s "$(command -v "$c")" "$bindir/$c"
done
print -r -- "$kitchen_block" > "$home/block.zsh"
zsh_bin=$(command -v zsh)
if HOME=$home MOUTH_WEIGHTS="models/normal" PATH="$bindir" "$zsh_bin" "$home/block.zsh"; then
  cfg=$home/.config/orbitty/mouth-sidecar.json
  leftovers=$(find "$home/.config" -name '.mouth-sidecar.json.*' -print 2>/dev/null || true)
  if [[ ! -e $cfg && ! -L $cfg && -z $leftovers ]]; then
    pass "kitchen_cfg_no_python3_no_file"
  else
    bad "kitchen_cfg_no_python3_no_file"
  fi
else
  bad "kitchen_cfg_no_python3_no_file"
fi

# ~/.config/orbitty is a file: block must not abort a following DONE line.
mk
home=$REPLY
mkdir -p "$home/Library/LaunchAgents" "$home/.config"
: > "$home/Library/LaunchAgents/space.orbitty.mouth.plist"
: > "$home/.config/orbitty"
{
  print -r -- "set -euo pipefail"
  print -r -- "$nightly_block"
  print -r -- "echo DONE"
} > "$home/runner.zsh"
if out=$(HOME=$home zsh "$home/runner.zsh" 2>&1); then
  if [[ $out == *DONE* ]]; then
    pass "nightly_cfg_dir_is_file_nonfatal"
  else
    bad "nightly_cfg_dir_is_file_nonfatal"
  fi
else
  bad "nightly_cfg_dir_is_file_nonfatal"
fi

exit $fail
