#!/bin/zsh
# Fixture tests for scripts/install_lib.sh and the kitchen script's
# no-write paths. Temp dirs only. No cargo, npm, codesign, or launchctl.
set -euo pipefail
umask 022

here=${0:A:h}
unset ORBITTY_MOUTH_WEIGHTS ORBITTY_SIGN_IDENTITY
source "$here/install_lib.sh"

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

fail=0
pass() { print -r -- "PASS $1"; }
bad() { print -r -- "FAIL $1"; fail=1; }

# --- clean_build_bundle ---

mk
root=$REPLY
mkdir -p "$root/src-tauri/target/release/bundle/macos/Orbitty.app/Contents"
print -r -- marker > "$root/src-tauri/target/release/bundle/macos/Orbitty.app/Contents/marker"
print -r -- sibling > "$root/src-tauri/target/release/bundle/macos/sibling"
if clean_build_bundle "$root"; then
  if [[ ! -e $root/src-tauri/target/release/bundle/macos/Orbitty.app \
        && -f $root/src-tauri/target/release/bundle/macos/sibling ]]; then
    pass "clean_removes_fixture_bundle"
  else
    bad "clean_removes_fixture_bundle"
  fi
else
  bad "clean_removes_fixture_bundle"
fi

mk
root=$REPLY
mkdir -p "$root/keep"
if clean_build_bundle "$root"; then
  if [[ -d $root/keep && ! -e $root/src-tauri ]]; then
    pass "clean_missing_is_noop"
  else
    bad "clean_missing_is_noop"
  fi
else
  bad "clean_missing_is_noop"
fi

mk
root=$REPLY
mk
victim=$REPLY
print -r -- secret > "$victim/secret"
mkdir -p "$root/src-tauri/target/release/bundle/macos"
ln -s "$victim" "$root/src-tauri/target/release/bundle/macos/Orbitty.app"
if clean_build_bundle "$root"; then
  bad "clean_refuses_outside_repo"
else
  if [[ -L $root/src-tauri/target/release/bundle/macos/Orbitty.app \
        && -f $victim/secret && $(<"$victim/secret") == secret ]]; then
    pass "clean_refuses_outside_repo"
  else
    bad "clean_refuses_outside_repo"
  fi
fi

mk
root=$REPLY
mkdir -p "$root/decoy-app"
print -r -- keep > "$root/decoy-app/keep"
mkdir -p "$root/src-tauri/target/release/bundle/macos"
ln -s "$root/decoy-app" "$root/src-tauri/target/release/bundle/macos/Orbitty.app"
if clean_build_bundle "$root"; then
  bad "clean_refuses_wrong_suffix"
else
  if [[ -L $root/src-tauri/target/release/bundle/macos/Orbitty.app \
        && -f $root/decoy-app/keep && $(<"$root/decoy-app/keep") == keep ]]; then
    pass "clean_refuses_wrong_suffix"
  else
    bad "clean_refuses_wrong_suffix"
  fi
fi

# --- warn_stale_path_ctl ---

mk
home=$REPLY
mk
binhome=$REPLY
bundled=$binhome/orbittyctl
print -r -- bundled-bytes > "$bundled"
out=$(warn_stale_path_ctl "$bundled" "$home")
if [[ -z $out && ! -e $home/.local/bin/orbittyctl ]]; then
  pass "stale_ctl_absent_silent"
else
  bad "stale_ctl_absent_silent"
fi

mk
home=$REPLY
mkdir -p "$home/.local/bin"
print -r -- same-bytes > "$home/.local/bin/orbittyctl"
bundled=$home/bundled-orbittyctl
print -r -- same-bytes > "$bundled"
before=$(<"$home/.local/bin/orbittyctl")
out=$(warn_stale_path_ctl "$bundled" "$home")
after=$(<"$home/.local/bin/orbittyctl")
if [[ $out == "ok: ~/.local/bin/orbittyctl matches the bundled orbittyctl" \
      && $before == "$after" ]] && cmp -s "$home/.local/bin/orbittyctl" "$bundled"; then
  pass "stale_ctl_identical_ok"
else
  bad "stale_ctl_identical_ok"
fi

mk
home=$REPLY
mkdir -p "$home/.local/bin"
print -r -- path-copy > "$home/.local/bin/orbittyctl"
bundled=$home/bundled-orbittyctl
print -r -- bundle-copy > "$bundled"
saved=$home/saved
cp "$home/.local/bin/orbittyctl" "$saved"
out=$(warn_stale_path_ctl "$bundled" "$home")
if [[ $out == *WARNING:\ stale\ PATH\ orbittyctl* \
      && $out == *Stable\'s\ socket* \
      && $out == *control.sock* \
      && $out == *--sock* \
      && $out == *scripts/install-kitchen.sh\ --cli* \
      && -f $home/.local/bin/orbittyctl ]] && cmp -s "$home/.local/bin/orbittyctl" "$saved"; then
  pass "stale_ctl_differs_warns_and_keeps_file"
else
  bad "stale_ctl_differs_warns_and_keeps_file"
fi

# --- resolve_sign_identity ---

got=$(resolve_sign_identity)
if [[ $got == "Orbitty Local Codesign" ]]; then
  pass "identity_default"
else
  bad "identity_default"
fi

got=$(ORBITTY_SIGN_IDENTITY="Widget Sign" resolve_sign_identity)
if [[ $got == "Widget Sign" ]]; then
  pass "identity_env_override"
else
  bad "identity_env_override"
fi

mk
home=$REPLY
if ORBITTY_SIGN_IDENTITY=- resolve_sign_identity >"$home/out" 2>"$home/err"; then
  bad "identity_adhoc_refused"
else
  if [[ ! -s $home/out ]] && grep -q "ad-hoc" "$home/err" && grep -q "Accessibility" "$home/err"; then
    pass "identity_adhoc_refused"
  else
    bad "identity_adhoc_refused"
  fi
fi

# --- kitchen_mouth_weights ---

write_plist() {
  local dest=$1 value=$2
  mkdir -p -- "${dest:h}"
  cat > "$dest" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>EnvironmentVariables</key>
  <dict>
    <key>ORBITTY_MOUTH_WEIGHTS</key>
    <string>$value</string>
  </dict>
</dict>
</plist>
EOF
}

mk
home=$REPLY
mkdir -p "$home/.config/orbitty"
print -r -- '{"weights":"from-json"}' > "$home/.config/orbitty/mouth-sidecar.json"
write_plist "$home/Library/LaunchAgents/space.orbitty.mouth.plist" "from-plist"
got=$(ORBITTY_MOUTH_WEIGHTS=from-env kitchen_mouth_weights "$home")
if [[ $got == from-env ]]; then
  pass "weights_env_wins"
else
  bad "weights_env_wins"
fi

mk
home=$REPLY
mkdir -p "$home/.config/orbitty"
print -r -- '{"weights":"from-json"}' > "$home/.config/orbitty/mouth-sidecar.json"
write_plist "$home/Library/LaunchAgents/space.orbitty.mouth.plist" "from-plist"
got=$(kitchen_mouth_weights "$home")
if [[ $got == from-json ]]; then
  pass "weights_from_config_json"
else
  bad "weights_from_config_json"
fi

mk
home=$REPLY
write_plist "$home/Library/LaunchAgents/space.orbitty.mouth.plist" "from-plist"
got=$(kitchen_mouth_weights "$home")
if [[ $got == from-plist ]]; then
  pass "weights_from_plist"
else
  bad "weights_from_plist"
fi

mk
home=$REPLY
got=$(kitchen_mouth_weights "$home")
if [[ -z $got ]]; then
  pass "weights_none_empty"
else
  bad "weights_none_empty"
fi

# --- kitchen script writes nothing ---

snapshot_empty() {
  local home=$1
  find "$home" -mindepth 1 -print
}

mk
home=$REPLY
if HOME=$home "$here/install-kitchen.sh" >"$home.out" 2>"$home.err"; then
  if [[ -z $(snapshot_empty "$home") ]]; then
    pass "kitchen_no_flags_writes_nothing"
  else
    bad "kitchen_no_flags_writes_nothing"
  fi
else
  bad "kitchen_no_flags_writes_nothing"
fi

mk
home=$REPLY
if HOME=$home "$here/install-kitchen.sh" --all --dry-run >"$home.out" 2>"$home.err"; then
  if [[ -z $(snapshot_empty "$home") ]] && grep -q "dry-run" "$home.out"; then
    pass "kitchen_dry_run_writes_nothing"
  else
    bad "kitchen_dry_run_writes_nothing"
  fi
else
  bad "kitchen_dry_run_writes_nothing"
fi

exit $fail
