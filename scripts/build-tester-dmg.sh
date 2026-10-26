#!/bin/zsh
# Build a friend alpha DMG from a clean main checkout.
# Identity space.orbitty.alpha (tauri.alpha.conf.json). Feature alpha, never
# dev-thin. Signs each Contents/MacOS helper, then the app, never --deep.
# The DMG is made here with hdiutil, then signed. Local signature, not notarized.
#
# Signing stays in this script. It is not moved into install_lib.sh.
# Cargo's bin name is orbitty-relay (hyphen). This script never creates
# orbitty_relay. If the bundler emits Contents/MacOS/orbitty_relay, delete it
# before any codesign so the DMG ships one relay binary.
#
# Writes only under ~/Orbitty Tester Builds/ or --out. Refuses /Applications,
# ~/Applications, and the repo. Never runs install-stable.sh or install-nightly.sh.
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

ALPHA_ID="space.orbitty.alpha"
ALPHA_CONF="$root/src-tauri/tauri.alpha.conf.json"
DATA_DIR="~/Library/Application Support/${ALPHA_ID}"
WANT_MACOS=(orbitty orbittyctl orbitty-mcp orbitty-relay orbitty-tabbity-host)

smoke=0
dry=0
force=0
out_dir=""
arch="arm64"

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
    --arch)
      if (( $# < 2 )); then
        print -r -- "error: --arch needs universal or arm64" >&2
        exit 1
      fi
      arch=$2
      shift 2
      ;;
    *)
      print -r -- "error: unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

if [[ $arch != arm64 && $arch != universal ]]; then
  print -r -- "error: --arch must be universal or arm64 (got $arch)" >&2
  exit 1
fi

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
if [[ -n $porcelain && smoke -eq 0 ]]; then
  print -r -- "error: working tree is dirty; refusing a non-smoke tester DMG" >&2
  exit 1
fi

head="$(git rev-parse HEAD)"
main_ref="$(git rev-parse main)"
if (( ! smoke )) && [[ $head != "$main_ref" ]]; then
  print -r -- "tester DMGs are built from main after the merge; use --smoke for a test build" >&2
  exit 1
fi
if [[ ! $head =~ '^[0-9a-f]{40}$' ]]; then
  print -r -- "error: HEAD is not a 40-hex commit ($head)" >&2
  exit 1
fi

if (( ! smoke )); then
  if ! python3 -c 'import re, sys
text = open(sys.argv[1], encoding="utf-8").read()
m = re.search(r"(?m)^\[features\]\s*$", text)
if not m:
    raise SystemExit(1)
rest = text[m.end():]
rest = re.split(r"(?m)^\[", rest, maxsplit=1)[0]
raise SystemExit(0 if re.search(r"(?m)^alpha\s*=", rest) else 1)
' "$root/src-tauri/Cargo.toml"; then
    print -r -- "error: src-tauri/Cargo.toml has no alpha feature; refusing a non-smoke DMG (ALPHA-FIRST-RUN). Use --smoke only for a harness build." >&2
    exit 1
  fi
  if ! command -v oxipng >/dev/null 2>&1; then
    print -r -- "error: oxipng is missing; refusing a non-smoke DMG" >&2
    exit 1
  fi
fi

if [[ ! -f $ALPHA_CONF ]]; then
  print -r -- "error: missing $ALPHA_CONF" >&2
  exit 1
fi
if ! python3 -c 'import json, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
ok = d.get("identifier") == "space.orbitty.alpha"
ok = ok and d.get("productName", "Orbitty") == "Orbitty"
raise SystemExit(0 if ok else 1)
' "$ALPHA_CONF"; then
  print -r -- "error: $ALPHA_CONF must set identifier to space.orbitty.alpha" >&2
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

# Resolve symlinks, including a final component that does not exist yet.
# Output inside the repo, /Applications, or ~/Applications is refused
# before any build, including --dry-run.
root_phys="$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$root")"
dest_phys="$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$dest_dir")"
home_apps="$(python3 -c 'import os; print(os.path.realpath(os.path.expanduser("~/Applications")))')"
case "${dest_phys}/" in
  "${root_phys}/"*)
    print -r -- "error: refusing output inside the repo ($dest_phys)" >&2
    exit 1
    ;;
  "/Applications/"*)
    print -r -- "error: refusing output under /Applications ($dest_phys)" >&2
    exit 1
    ;;
  "${home_apps}/"*)
    print -r -- "error: refusing output under ~/Applications ($dest_phys)" >&2
    exit 1
    ;;
esac

if (( smoke )); then
  dmg_name="Orbitty-${version}-alpha-${sha7}-${arch}-SMOKE.dmg"
else
  dmg_name="Orbitty-${version}-alpha-${sha7}-${arch}.dmg"
fi
out="${dest_dir}/${dmg_name}"

if [[ -e $out && $force -eq 0 ]]; then
  print -r -- "error: $out already exists; pass --force to overwrite" >&2
  exit 1
fi

if [[ $arch == universal ]]; then
  target_flag=(--target universal-apple-darwin)
  app="$root/src-tauri/target/universal-apple-darwin/release/bundle/macos/Orbitty.app"
  lipo_expect="x86_64 arm64"
  arch_line="architectures: x86_64 arm64"
else
  target_flag=()
  app="$root/src-tauri/target/release/bundle/macos/Orbitty.app"
  lipo_expect="arm64"
  arch_line="architectures: Apple silicon Macs (M1 or newer) only"
fi

cargo_cmd=(cargo build --offline --locked --release --features alpha
  --bin orbittyctl --bin orbitty-mcp --bin orbitty-relay --bin orbitty-tabbity-host)
cargo_cmd+=("${target_flag[@]}")
tauri_cmd=(npm --offline run tauri build -- --config "$ALPHA_CONF" --features alpha --bundles app)
tauri_cmd+=("${target_flag[@]}")
tauri_cmd+=(-- --locked --offline)

for word in "${cargo_cmd[@]}" "${tauri_cmd[@]}"; do
  if [[ $word == *dev-thin* ]]; then
    print -r -- "error: refusing a dev-thin build" >&2
    exit 1
  fi
done

print_plan() {
  print -r -- "dry-run: identity: $IDENTITY"
  print -r -- "dry-run: version: $version"
  print -r -- "dry-run: sha7: $sha7"
  print -r -- "dry-run: arch: $arch"
  print -r -- "dry-run: bundle id: $ALPHA_ID"
  print -r -- "dry-run: would write: $out"
  print -r -- "dry-run: would run:"
  print -r -- "  (cd src-tauri && ${cargo_cmd[*]})"
  print -r -- "  clean_build_bundle"
  print -r -- "  ${tauri_cmd[*]}"
  print -r -- "  rm Contents/MacOS/orbitty_relay before signing"
  print -r -- "  oxipng -o 4 --strip safe on staged PNGs"
  print -r -- "  codesign --force --sign $IDENTITY --timestamp=none each Contents/MacOS helper (glob, not orbitty)"
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

# Drop Matt's keys from the build environment. Locals already captured
# identity, dest, and head. Do not print values.
scrub_build_env() {
  unset OPENROUTER_API_KEY
  local name
  for name in ${(k)parameters}; do
    case $name in
      ORBITTY_*|*_API_KEY)
        unset "$name"
        ;;
    esac
  done
}
scrub_build_env

print -r -- "==> Building helpers (${arch})"
(cd "$root/src-tauri" && "${cargo_cmd[@]}")

print -r -- "==> Cleaning build-tree bundle"
if [[ $arch == universal ]]; then
  if [[ -e $app || -L $app ]]; then
    rm -rf -- "$app"
  fi
else
  clean_build_bundle "$root"
fi

print -r -- "==> Building alpha app bundle (no dev-thin)"
"${tauri_cmd[@]}"

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
  want=("${WANT_MACOS[@]}")
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
    print -r -- "error: Contents/MacOS must be exactly ${(j:, :)want}" >&2
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

print -r -- "==> oxipng staged PNGs (lossless)"
pngs=("$app"/**/*.png(N))
if (( ${#pngs} )); then
  oxipng -o 4 --strip safe -- "${pngs[@]}"
fi

print -r -- "==> Scanning staged app for keys and forbidden names"
# Prints file:offset and the prefix only. Never the match.
if ! python3 -c 'import os, re, sys
root = sys.argv[1]
exact = {"settings.json", "go-runner.json", "mouth-sidecar.json"}
key_re = re.compile(br"(sk-or-v1-|sk-ant-|sk-proj-|xai-|AIza|gh[pousr]_)([A-Za-z0-9_-]{20,})")
failed = False
for dirpath, dirnames, filenames in os.walk(root):
    dirnames.sort()
    for name in sorted(filenames):
        path = os.path.join(dirpath, name)
        if name.startswith(".env") or name.endswith(".key") or name.endswith(".pem") or name.startswith("openrouter") or name in exact:
            print("forbidden name: %s" % path, file=sys.stderr)
            failed = True
        try:
            data = open(path, "rb").read()
        except OSError:
            print("forbidden unreadable: %s" % path, file=sys.stderr)
            failed = True
            continue
        for m in key_re.finditer(data):
            prefix = m.group(1).decode("ascii", "replace")
            print("%s:%d prefix=%s" % (path, m.start(), prefix))
            failed = True
sys.exit(1 if failed else 0)
' "$app"; then
  print -r -- "error: staged app failed the secret scan; refusing to sign" >&2
  exit 1
fi

user_hits=0
if command -v strings >/dev/null 2>&1; then
  for f in "$macos"/*(N.); do
    n="$(strings -a "$f" | grep -c -E '/Users/[^/[:space:]]+' || true)"
    n="${n//[^0-9]/}"
    user_hits=$(( user_hits + ${n:-0} ))
  done
fi
print -r -- "strings /Users/<name> hits: $user_hits (reported, not a failure)"

plist="$app/Contents/Info.plist"
if [[ ! -f $plist ]]; then
  print -r -- "error: missing $plist" >&2
  exit 1
fi
dirty_bit=false
if [[ -n $porcelain ]]; then
  dirty_bit=true
fi
/usr/libexec/PlistBuddy -c "Set :OrbittyGitCommit $head" "$plist" 2>/dev/null \
  || /usr/libexec/PlistBuddy -c "Add :OrbittyGitCommit string $head" "$plist"
/usr/libexec/PlistBuddy -c "Set :OrbittyGitDirty $dirty_bit" "$plist" 2>/dev/null \
  || /usr/libexec/PlistBuddy -c "Add :OrbittyGitDirty bool $dirty_bit" "$plist"

got_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$plist" 2>/dev/null || true)"
if [[ $got_id != "$ALPHA_ID" ]]; then
  print -r -- "error: CFBundleIdentifier is '${got_id:-<missing>}', want $ALPHA_ID (config merge, not a plist rename)" >&2
  exit 1
fi

macos_min="$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$plist" 2>/dev/null || true)"
if [[ -z $macos_min ]]; then
  macos_min="unknown"
fi

print -r -- "==> lipo -archs (expect $lipo_expect)"
for f in "$macos"/*(N.); do
  got="$(lipo -archs "$f" 2>/dev/null || true)"
  if [[ $got != "$lipo_expect" ]]; then
    print -r -- "error: ${f:t} lipo -archs is '${got:-<none>}', want $lipo_expect" >&2
    exit 1
  fi
done

sign_one() {
  local target=$1
  print -r -- "codesign --force --sign $IDENTITY --timestamp=none $target"
  codesign --force --sign "$IDENTITY" --timestamp=none "$target"
}

print -r -- "==> Signing helpers, then the app (no --deep)"
for f in "$macos"/*(N.); do
  if [[ ${f:t} == orbitty ]]; then
    continue
  fi
  sign_one "$f"
done
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
for f in "$macos"/*(N.); do
  if [[ ${f:t} == orbitty ]]; then
    continue
  fi
  authority_ok "$f"
done

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
if [[ -f $root/README.md ]]; then
  cp -- "$root/README.md" "$staging/README.md"
fi
cat > "$staging/BUILD_INFO.txt" <<EOF
version: $version
sha: $head
sha7: $sha7
date: $today
identity: $IDENTITY
bundle id: $ALPHA_ID
data dir: $DATA_DIR
$arch_line
macos minimum: $macos_min
not notarized
alpha, companion in spirit form only (no faces)
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
