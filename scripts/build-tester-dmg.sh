#!/bin/zsh
# Build a tester DMG from a clean main checkout.
# Stable app only (no dev-thin). Signs each Contents/MacOS helper, then the
# app, never --deep. The DMG is made here with hdiutil, then signed.
#
# Signing stays in this script (same flags as install-stable's
# sign_macos_helpers_then_app). It is not moved into install_lib.sh, so
# install-stable and install-nightly keep their own copies and behavior.
#
# Cargo's bin name is orbitty-relay (hyphen). Tauri's bundler also copies
# target/release/orbitty_relay and exits 1 if that path is missing. This
# script makes the symlink the bundler requires, then deletes
# Contents/MacOS/orbitty_relay before any codesign so the DMG ships one
# relay binary.
set -euo pipefail

unset CARGO_TARGET_DIR
export CARGO_NET_OFFLINE=true

cd "${0:A:h}/.."
root=$PWD

# A test harness puts stub cargo/npm/codesign ahead of this. Do not bury them.
if ! command -v cargo >/dev/null 2>&1; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

source "$root/scripts/install_lib.sh"

smoke=0
dry=0
force=0
out_dir=""

while (( $# )); do
  case $1 in
    --smoke)
      smoke=1
      shift
      ;;
    --dry-run)
      dry=1
      shift
      ;;
    --force)
      force=1
      shift
      ;;
    --out)
      if (( $# < 2 )); then
        print -r -- "error: --out needs a directory" >&2
        exit 1
      fi
      out_dir=$2
      shift 2
      ;;
    *)
      print -r -- "error: unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

# Ad-hoc (-) is refused inside resolve_sign_identity, before any build.
if ! IDENTITY="$(resolve_sign_identity)"; then
  exit 1
fi

# Capture-then-grep, same as install-stable: a pipe to grep -q races SIGPIPE.
IDS="$(security find-identity -v -p codesigning)" || true
if ! grep -F -q -e "$IDENTITY" <<< "$IDS"; then
  print -r -- "ERROR: \"$IDENTITY\" not found by:" >&2
  print -r -- "  security find-identity -v -p codesigning" >&2
  print -r -- "Refusing to build. An ad-hoc fallback is refused." >&2
  exit 1
fi

porcelain="$(git status --porcelain)"
if [[ -n $porcelain ]]; then
  print -r -- "error: working tree is dirty; refusing to build a tester DMG" >&2
  exit 1
fi

head="$(git rev-parse HEAD)"
main_ref="$(git rev-parse main)"
if (( ! smoke )) && [[ $head != "$main_ref" ]]; then
  print -r -- "tester DMGs are built from main after the merge; use --smoke for a test build" >&2
  exit 1
fi

version="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["version"])' "$root/src-tauri/tauri.conf.json")"
if [[ -z $version ]]; then
  print -r -- "error: empty version in src-tauri/tauri.conf.json" >&2
  exit 1
fi
sha7="${head:0:7}"
today="$(date +%Y-%m-%d)"

if [[ -n $out_dir ]]; then
  dest_dir=$out_dir
else
  dest_dir="${ORBITTY_DMG_OUT:-$HOME/Orbitty Tester Builds}"
fi

if (( smoke )); then
  dmg_name="Orbitty-${version}-alpha-${sha7}-SMOKE.dmg"
else
  dmg_name="Orbitty-${version}-alpha-${sha7}.dmg"
fi
out="${dest_dir}/${dmg_name}"

if [[ -e $out && $force -eq 0 ]]; then
  print -r -- "error: $out already exists; pass --force to overwrite" >&2
  exit 1
fi

app="$root/src-tauri/target/release/bundle/macos/Orbitty.app"

print_plan() {
  print -r -- "dry-run: identity: $IDENTITY"
  print -r -- "dry-run: version: $version"
  print -r -- "dry-run: sha7: $sha7"
  print -r -- "dry-run: would write: $out"
  print -r -- "dry-run: would run:"
  print -r -- "  (cd src-tauri && cargo build --offline --release --bin orbittyctl --bin orbitty-mcp --bin orbitty-relay)"
  print -r -- "  ln -sfn orbitty-relay src-tauri/target/release/orbitty_relay"
  print -r -- "  clean_build_bundle"
  print -r -- "  npm --offline run tauri build -- --bundles app"
  print -r -- "  rm Contents/MacOS/orbitty_relay before signing"
  print -r -- "  codesign --force --sign $IDENTITY --timestamp=none Contents/MacOS/orbittyctl"
  print -r -- "  codesign --force --sign $IDENTITY --timestamp=none Contents/MacOS/orbitty-mcp"
  print -r -- "  codesign --force --sign $IDENTITY --timestamp=none Contents/MacOS/orbitty-relay"
  print -r -- "  codesign --force --sign $IDENTITY --timestamp=none Orbitty.app"
  print -r -- "  hdiutil create -volname Orbitty -fs HFS+ -format UDZO -srcfolder <staging> $out"
  print -r -- "  codesign --sign $IDENTITY --timestamp=none $out"
  print -r -- "  hdiutil verify $out"
  print -r -- "dry-run: writes nothing"
}

if (( dry )); then
  print_plan
  exit 0
fi

print -r -- "==> Building orbittyctl, orbitty-mcp, orbitty-relay"
(cd "$root/src-tauri" && cargo build --offline --release --bin orbittyctl --bin orbitty-mcp --bin orbitty-relay)

# The bundler copies this underscore path and fails if it is absent.
# It is not a second cargo bin. The bundled copy is removed before signing.
print -r -- "ln -sfn orbitty-relay src-tauri/target/release/orbitty_relay"
mkdir -p -- "$root/src-tauri/target/release"
ln -sfn orbitty-relay "$root/src-tauri/target/release/orbitty_relay"

print -r -- "==> Cleaning build-tree bundle"
clean_build_bundle "$root"

print -r -- "==> Building Stable app bundle (no dev-thin)"
npm --offline run tauri build -- --bundles app

if [[ ! -d $app ]]; then
  print -r -- "error: expected bundle at $app" >&2
  exit 1
fi

macos="$app/Contents/MacOS"
print -r -- "Contents/MacOS before relay fixup:"
for f in "$macos"/*(N); do
  print -r -- "  ${f:t}"
done

relay="$macos/orbitty_relay"
if [[ -e $relay || -L $relay ]]; then
  rm -f -- "$relay"
  print -r -- "removed Contents/MacOS/orbitty_relay"
fi

require_exact_macos() {
  local name w ok
  local -a names unexpected missing want
  local -A seen
  names=()
  unexpected=()
  missing=()
  want=(orbitty orbittyctl orbitty-mcp orbitty-relay)
  seen=()
  for f in "$macos"/*(N); do
    name=${f:t}
    names+=("$name")
    seen[$name]=1
  done
  for name in "${names[@]}"; do
    ok=0
    for w in "${want[@]}"; do
      if [[ $name == "$w" ]]; then
        ok=1
        break
      fi
    done
    if (( ! ok )); then
      unexpected+=("$name")
    fi
  done
  for w in "${want[@]}"; do
    if (( ${+seen[$w]} == 0 )); then
      missing+=("$w")
    fi
  done
  for w in "${want[@]}"; do
    if [[ -L $macos/$w ]]; then
      print -r -- "error: $w is a symlink" >&2
      return 1
    fi
  done
  if (( ${#unexpected} || ${#missing} )); then
    print -r -- "error: Contents/MacOS must be exactly orbitty, orbittyctl, orbitty-mcp, orbitty-relay" >&2
    if (( ${#unexpected} )); then
      print -r -- "error: unexpected: ${(j:, :)unexpected}" >&2
    fi
    if (( ${#missing} )); then
      print -r -- "error: missing: ${(j:, :)missing}" >&2
    fi
    return 1
  fi
  return 0
}

require_exact_macos

print -r -- "==> Checking bundled orbittyctl"
"$root/scripts/check_bundled_ctl.sh" "$app"

sign_one() {
  local target=$1
  print -r -- "codesign --force --sign $IDENTITY --timestamp=none $target"
  codesign --force --sign "$IDENTITY" --timestamp=none "$target"
}

print -r -- "==> Signing helpers, then the app (no --deep)"
sign_one "$macos/orbittyctl"
sign_one "$macos/orbitty-mcp"
sign_one "$macos/orbitty-relay"
sign_one "$app"

print -r -- "==> Verifying signature"
codesign --verify --deep --strict --verbose=2 "$app"

authority_ok() {
  local target=$1 sig
  sig="$(codesign -dvv "$target" 2>&1)" || true
  if ! grep -F -x -q -- "Authority=$IDENTITY" <<< "$sig"; then
    print -r -- "ERROR: $target is not signed by \"$IDENTITY\"" >&2
    print -r -- "$sig" >&2
    return 1
  fi
  print -r -- "signed: $target"
  grep -E '^(Identifier|Authority|TeamIdentifier)=' <<< "$sig" || true
}

authority_ok "$app"
authority_ok "$macos/orbittyctl"
authority_ok "$macos/orbitty-mcp"
authority_ok "$macos/orbitty-relay"

print -r -- "==> spctl -a -vv (rejection is expected; not a failure)"
spctl_out="$(spctl -a -vv "$app" 2>&1)" || true
if [[ -n $spctl_out ]]; then
  print -r -- "$spctl_out"
fi
print -r -- "spctl: recorded (rejection expected for a local signature)"

staging="$(mktemp -d "${TMPDIR:-/tmp}/orbitty-tester-dmg.XXXXXX")"
trap 'rm -rf -- "$staging"' EXIT

print -r -- "==> Staging DMG contents"
ditto "$app" "$staging/Orbitty.app"
ln -s /Applications "$staging/Applications"
cp -- "$root/docs/TESTER_OPEN_ANYWAY.md" "$staging/Open Anyway - read me first.md"
cat > "$staging/BUILD_INFO.txt" <<EOF
version: $version
sha: $head
sha7: $sha7
date: $today
identity: $IDENTITY
alpha, no companion
EOF

mkdir -p -- "$dest_dir"
if [[ -e $out ]]; then
  rm -f -- "$out"
fi

print -r -- "==> Creating DMG"
print -r -- "hdiutil create -volname Orbitty -fs HFS+ -format UDZO -srcfolder $staging $out"
hdiutil create -volname "Orbitty" -fs "HFS+" -format UDZO -srcfolder "$staging" "$out"

print -r -- "==> Signing DMG"
print -r -- "codesign --sign $IDENTITY --timestamp=none $out"
codesign --sign "$IDENTITY" --timestamp=none "$out"

print -r -- "==> Verifying DMG"
hdiutil verify "$out"

bytes="$(stat -f '%z' "$out")"
sum="$(shasum -a 256 "$out" | awk 'NR==1 { print $1 }')"
print -r -- "dmg: $out"
print -r -- "size: $bytes"
print -r -- "sha256: $sum"
