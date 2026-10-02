#!/bin/zsh
# Shared helpers for the Orbitty install scripts.
# Functions only: sourcing this file has no side effects.

# Remove the build-tree Orbitty.app. Refuses unless the resolved path is
# inside <repo_root> and is exactly
# <repo_root>/src-tauri/target/release/bundle/macos/Orbitty.app.
# A missing bundle is a no-op (returns 0). Never deletes anything else.
clean_build_bundle() {
  local root=${1:-}
  local root_phys suffix bundle resolved
  if [[ -z $root || ! -d $root ]]; then
    print -r -- "clean_build_bundle: repo root is not a directory: ${root:-<empty>}" >&2
    return 1
  fi
  root_phys=$(cd -- "$root" && pwd -P) || {
    print -r -- "clean_build_bundle: cannot resolve repo root: $root" >&2
    return 1
  }
  suffix="/src-tauri/target/release/bundle/macos/Orbitty.app"
  bundle="${root_phys}${suffix}"
  if [[ ! -e $bundle && ! -L $bundle ]]; then
    return 0
  fi
  # A dangling symlink is the link itself (inside the repo). Removing it
  # does not follow a target.
  if [[ -L $bundle && ! -e $bundle ]]; then
    rm -f -- "$bundle"
    return 0
  fi
  if [[ ! -d $bundle ]]; then
    print -r -- "clean_build_bundle: refused (not a directory): $bundle" >&2
    return 1
  fi
  resolved=$(cd -- "$bundle" && pwd -P) || {
    print -r -- "clean_build_bundle: cannot resolve $bundle" >&2
    return 1
  }
  if [[ $resolved != ${root_phys}${suffix} ]]; then
    case $resolved in
      ${root_phys}/*)
        print -r -- "clean_build_bundle: refused path with wrong suffix: $resolved" >&2
        ;;
      *)
        print -r -- "clean_build_bundle: refused path outside repo: $resolved" >&2
        ;;
    esac
    return 1
  fi
  rm -rf -- "$bundle"
}

# Warn when ~/.local/bin/orbittyctl exists and is not the bundled binary.
# Never deletes, moves, or overwrites. Always returns 0.
# <home> defaults to $HOME (tests pass a temp home).
warn_stale_path_ctl() {
  local bundled=${1:-}
  local home=${2:-$HOME}
  local path_ctl="$home/.local/bin/orbittyctl"
  if [[ ! -e $path_ctl && ! -L $path_ctl ]]; then
    return 0
  fi
  if [[ -n $bundled && -e $bundled && -e $path_ctl ]] && cmp -s -- "$path_ctl" "$bundled"; then
    print -r -- "ok: ~/.local/bin/orbittyctl matches the bundled orbittyctl"
    return 0
  fi
  cat <<'EOF'
WARNING: stale PATH orbittyctl
WARNING: ~/.local/bin/orbittyctl differs from the bundled orbittyctl and talks to Stable's socket (~/Library/Application Support/space.orbitty.mac/control.sock) unless given --sock
WARNING: refresh it with scripts/install-kitchen.sh --cli or remove it yourself
EOF
  return 0
}

# Echo the codesign identity. ORBITTY_SIGN_IDENTITY=- (ad-hoc) is refused.
resolve_sign_identity() {
  local id=${ORBITTY_SIGN_IDENTITY:-Orbitty Local Codesign}
  if [[ $id == "-" ]]; then
    print -r -- "ERROR: ad-hoc signing (ORBITTY_SIGN_IDENTITY=-) is refused. An ad-hoc signature breaks the Accessibility grant: an ad-hoc CDHash changes every rebuild." >&2
    return 1
  fi
  print -r -- "$id"
}

# First non-empty mouth weights: $ORBITTY_MOUTH_WEIGHTS, then weights in
# <home>/.config/orbitty/mouth-sidecar.json, then the existing mouth plist.
# Echoes nothing when none of those are set.
kitchen_mouth_weights() {
  local home=${1:-$HOME}
  local cfg plist w
  if [[ -n ${ORBITTY_MOUTH_WEIGHTS:-} ]]; then
    print -r -- "$ORBITTY_MOUTH_WEIGHTS"
    return 0
  fi
  cfg="$home/.config/orbitty/mouth-sidecar.json"
  if [[ -f $cfg ]]; then
    w=$(python3 -c 'import json,sys
try:
    d=json.load(open(sys.argv[1]))
except Exception:
    raise SystemExit(0)
w=d.get("weights") if isinstance(d, dict) else None
if isinstance(w, str) and w != "":
    sys.stdout.write(w)
' "$cfg" 2>/dev/null) || true
    if [[ -n $w ]]; then
      print -r -- "$w"
      return 0
    fi
  fi
  plist="$home/Library/LaunchAgents/space.orbitty.mouth.plist"
  if [[ -f $plist ]]; then
    w=$(/usr/libexec/PlistBuddy -c 'Print :EnvironmentVariables:ORBITTY_MOUTH_WEIGHTS' "$plist" 2>/dev/null) || true
    if [[ -n $w ]]; then
      print -r -- "$w"
      return 0
    fi
  fi
  return 0
}
