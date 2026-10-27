#!/bin/zsh
# Behavioral tests for scripts/build-tester-dmg.sh.
# Stubs on PATH. The real script runs in a temp git-init'd copy of the
# repo (removed on exit). Nothing in the checkout is written or deleted.
# No source grep.
#
# Mutation that must fail each case (apply to build-tester-dmg.sh, re-run):
#   T1  skip resolve_sign_identity (accept ORBITTY_SIGN_IDENTITY=-)
#   T2  move the security find-identity check to after cargo
#   T3  drop the HEAD == main check
#   T4  re-add the orbitty_relay symlink (or remove it only after codesign)
#   T5  add --deep to the Orbitty.app codesign --sign
#   T6  skip require_exact_macos
#   T7  skip codesign of the dmg
#   T8  run the build from --dry-run
#   T9  dry-run creates the relay symlink with /bin/ln
#   T10 authority check always passes
#   T11 remove codesign --verify --deep --strict
#   T12 remove hdiutil verify
#   T13 remove the --force overwrite guard
#   T14 remove the helper-is-symlink guard
#   T15 skip check_bundled_ctl.sh
#   T16 drop the inside-repo --out refusal
#   T17 drop --config / allow dev-thin / skip the plist identifier check
#   T18 drop the alpha-feature refusal
#   T19 hardcode the old three helpers (skip orbitty-tabbity-host)
#   T20 drop the oxipng-missing refusal
#   T21 drop the secret scan
#   T22 drop --target universal-apple-darwin or the per-file lipo check
#   T23 allow a dirty non-smoke build
#   T24 allow --out under /Applications or ~/Applications
set -euo pipefail

here=${0:A:h}
root=${here:h}

fail=0
pass() { print -r -- "PASS $1"; }
bad() { print -r -- "FAIL $1"; fail=1; }

tmp="$(mktemp -d "${TMPDIR:-/tmp}/orbitty-test-dmg.XXXXXX")"
trap 'rm -rf -- "$tmp"' EXIT

sandbox="$tmp/repo"
mkdir -p -- "$sandbox/scripts" "$sandbox/docs" "$sandbox/src-tauri"
cp -- "$root/scripts/build-tester-dmg.sh" "$sandbox/scripts/build-tester-dmg.sh"
cp -- "$root/scripts/install_lib.sh" "$sandbox/scripts/install_lib.sh"
cp -- "$root/scripts/check_bundled_ctl.sh" "$sandbox/scripts/check_bundled_ctl.sh"
cp -- "$root/docs/TESTER_OPEN_ANYWAY.md" "$sandbox/docs/TESTER_OPEN_ANYWAY.md"
cp -- "$root/src-tauri/tauri.conf.json" "$sandbox/src-tauri/tauri.conf.json"
cp -- "$root/src-tauri/tauri.alpha.conf.json" "$sandbox/src-tauri/tauri.alpha.conf.json"
cat > "$sandbox/src-tauri/Cargo.toml" <<'EOF'
[features]
alpha = []
dev-thin = []
EOF
chmod 755 "$sandbox/scripts/build-tester-dmg.sh" "$sandbox/scripts/check_bundled_ctl.sh"
git -C "$sandbox" init -q
script="$sandbox/scripts/build-tester-dmg.sh"

STUB_BIN="$tmp/bin"
STUB_DIR="$tmp/state"
STUB_LOG="$STUB_DIR/calls.log"
mkdir -p "$STUB_BIN" "$STUB_DIR"
export STUB_LOG STUB_DIR

HEAD_OK="abcdef1234567890abcdef1234567890abcdef12"
export STUB_HEAD="$HEAD_OK"
export STUB_MAIN="$HEAD_OK"
export STUB_PORCELAIN=""
export STUB_SECURITY_OUT='  1) C0FFEE "Orbitty Local Codesign"'
export STUB_AUTHORITY="Orbitty Local Codesign"
export ORBITTY_TEST_EXTRA=""
export PYTHONDONTWRITEBYTECODE=1

cc -o "$tmp/ctl" -x c - <<'EOF'
#include <stdio.h>
int main(void) {
    fputs("usage: orbittyctl [--json]\n", stdout);
    return 0;
}
EOF

fixture="$tmp/fixture.app"
mkdir -p "$fixture/Contents/MacOS"
for name in orbitty orbittyctl orbitty-mcp orbitty-relay orbitty-tabbity-host orbitty_relay; do
  cp "$tmp/ctl" "$fixture/Contents/MacOS/$name"
  chmod 755 "$fixture/Contents/MacOS/$name"
done
cat > "$fixture/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleIdentifier</key><string>space.orbitty.alpha</string>
  <key>LSMinimumSystemVersion</key><string>15.0</string>
</dict></plist>
EOF
print -r -- "$fixture" > "$STUB_DIR/fixture_path"
export ORBITTY_TEST_FIXTURE="$fixture"

cat > "$STUB_BIN/git" <<'EOF'
#!/bin/zsh
print -r -- "git $*" >> "$STUB_LOG"
if [[ $1 == status ]]; then
  if [[ -n ${STUB_PORCELAIN:-} ]]; then
    print -r -- "$STUB_PORCELAIN"
  fi
  exit 0
fi
if [[ $1 == rev-parse && $2 == HEAD ]]; then
  print -r -- "$STUB_HEAD"
  exit 0
fi
if [[ $1 == rev-parse && $2 == main ]]; then
  print -r -- "$STUB_MAIN"
  exit 0
fi
print -r -- "git stub: unhandled $*" >&2
exit 1
EOF

cat > "$STUB_BIN/security" <<'EOF'
#!/bin/zsh
print -r -- "security $*" >> "$STUB_LOG"
print -r -- "${STUB_SECURITY_OUT-}"
exit 0
EOF

cat > "$STUB_BIN/cargo" <<'EOF'
#!/bin/zsh
print -r -- "cargo $*" >> "$STUB_LOG"
if [[ -n ${OPENROUTER_API_KEY:-} || -n ${SOME_API_KEY:-} ]]; then
  print -r -- "KEY_SET" >> "$STUB_LOG"
else
  print -r -- "KEY_UNSET" >> "$STUB_LOG"
fi
if [[ -n ${ORBITTY_LEAK:-} ]]; then
  print -r -- "ORBITTY_SET" >> "$STUB_LOG"
else
  print -r -- "ORBITTY_UNSET" >> "$STUB_LOG"
fi
exit 0
EOF

cat > "$STUB_BIN/npm" <<'EOF'
#!/bin/zsh
print -r -- "npm $*" >> "$STUB_LOG"
dest="src-tauri/target/release/bundle/macos/Orbitty.app"
if [[ " $* " == *"universal-apple-darwin"* ]]; then
  dest="src-tauri/target/universal-apple-darwin/release/bundle/macos/Orbitty.app"
fi
fixture="${ORBITTY_TEST_FIXTURE:-}"
if [[ -z $fixture && -f ${STUB_DIR:-}/fixture_path ]]; then
  fixture="$(<"$STUB_DIR/fixture_path")"
fi
rm -rf -- "$dest"
mkdir -p -- "${dest:h}"
cp -R -- "$fixture" "$dest"
extra="${ORBITTY_TEST_EXTRA:-}"
if [[ -z $extra && -f ${STUB_DIR:-}/extra ]]; then
  extra="$(<"$STUB_DIR/extra")"
fi
if [[ -n $extra ]]; then
  : > "$dest/Contents/MacOS/$extra"
fi
link="${ORBITTY_TEST_SYMLINK_HELPER:-}"
if [[ -z $link && -f ${STUB_DIR:-}/symlink_helper ]]; then
  link="$(<"$STUB_DIR/symlink_helper")"
fi
if [[ -n $link ]]; then
  helper="$dest/Contents/MacOS/$link"
  rm -f -- "$helper"
  ln -s orbitty "$helper"
fi
bad="${ORBITTY_TEST_BAD_CTL:-}"
if [[ -z $bad && -f ${STUB_DIR:-}/bad_ctl ]]; then
  bad=1
fi
if [[ -n $bad ]]; then
  printf '%s\n' '#!/bin/sh' 'exit 0' > "$dest/Contents/MacOS/orbittyctl"
  chmod 755 "$dest/Contents/MacOS/orbittyctl"
fi
plant=""
if [[ -f ${STUB_DIR:-}/plant ]]; then
  plant="$(<"$STUB_DIR/plant")"
fi
mkdir -p -- "$dest/Contents/Resources"
case $plant in
  env) print -r -- "nope" > "$dest/Contents/Resources/.env" ;;
  key) print -r -- "nope" > "$dest/Contents/Resources/x.key" ;;
  pem) print -r -- "nope" > "$dest/Contents/Resources/x.pem" ;;
  settings) print -r -- "{}" > "$dest/Contents/Resources/settings.json" ;;
  secret) print -r -- "sk-or-v1-TESTTESTTESTTESTTEST" > "$dest/Contents/Resources/notes.txt" ;;
esac
exit 0
EOF

cat > "$STUB_BIN/codesign" <<'EOF'
#!/bin/zsh
print -r -- "codesign $*" >> "$STUB_LOG"
target=${@[-1]}
if [[ " $* " == *" --sign "* ]]; then
  if [[ ! -f $STUB_DIR/saw_sign ]]; then
    macos=""
    if [[ $target == *.app ]]; then
      macos="$target/Contents/MacOS"
    elif [[ $target != *.dmg ]]; then
      macos=${target:h}
    fi
    if [[ -n $macos ]]; then
      if [[ -e $macos/orbitty_relay || -L $macos/orbitty_relay ]]; then
        print -r -- "RELAY_PRESENT" >> "$STUB_LOG"
      else
        print -r -- "RELAY_ABSENT" >> "$STUB_LOG"
      fi
    fi
    : > "$STUB_DIR/saw_sign"
  fi
  exit 0
fi
if [[ $1 == --verify ]]; then
  if [[ -n ${STUB_VERIFY_FAIL:-} ]]; then
    exit 1
  fi
  exit 0
fi
if [[ $1 == -dvv ]]; then
  base=${target:t}
  if [[ -n ${STUB_BAD_AUTHORITY_FOR:-} && $base == "$STUB_BAD_AUTHORITY_FOR" ]]; then
    print -r -- "Authority=Other Signer"
  else
    print -r -- "Authority=${STUB_AUTHORITY}"
  fi
  exit 0
fi
exit 0
EOF

cat > "$STUB_BIN/hdiutil" <<'EOF'
#!/bin/zsh
print -r -- "hdiutil $*" >> "$STUB_LOG"
if [[ $1 == create ]]; then
  src=""
  args=("$@")
  i=1
  while (( i <= $#args )); do
    if [[ ${args[$i]} == -srcfolder ]]; then
      src=${args[$((i + 1))]}
    fi
    i=$((i + 1))
  done
  out=${@[-1]}
  if [[ -z $src || ! -d $src ]]; then
    print -r -- "hdiutil stub: missing -srcfolder" >&2
    exit 1
  fi
  python3 -c 'import json,os,sys
src=sys.argv[1]
out=sys.argv[2]
entries=[]
for name in sorted(os.listdir(src)):
    p=os.path.join(src, name)
    if os.path.islink(p):
        entries.append({"name": name, "symlink": os.readlink(p)})
    elif os.path.isdir(p):
        entries.append({"name": name, "dir": True})
    else:
        entries.append({"name": name, "file": os.path.getsize(p)})
macos=os.path.join(src, "Orbitty.app", "Contents", "MacOS")
names=sorted(os.listdir(macos)) if os.path.isdir(macos) else []
guide=os.path.join(src, "Open Anyway - read me first.md")
info=os.path.join(src, "BUILD_INFO.txt")
plist=os.path.join(src, "Orbitty.app", "Contents", "Info.plist")
snap={"entries": entries, "macos": names}
if os.path.isfile(guide):
    snap["guide"]=open(guide, encoding="utf-8").read()
if os.path.isfile(info):
    snap["info"]=open(info, encoding="utf-8").read()
if os.path.isfile(plist):
    import plistlib
    pl=plistlib.loads(open(plist, "rb").read())
    snap["plist_id"]=pl.get("CFBundleIdentifier")
    snap["plist_commit"]=pl.get("OrbittyGitCommit")
    snap["plist_dirty"]=pl.get("OrbittyGitDirty")
open(out, "w", encoding="utf-8").write(json.dumps(snap))
' "$src" "$STUB_DIR/stage.json"
  mkdir -p -- "${out:h}"
  print -r -- "fake-dmg" > "$out"
  exit 0
fi
if [[ $1 == verify ]]; then
  if [[ -n ${STUB_HDIUTIL_VERIFY_FAIL:-} ]]; then
    exit 1
  fi
  exit 0
fi
exit 0
EOF

cat > "$STUB_BIN/ln" <<'EOF'
#!/bin/zsh
print -r -- "ln $*" >> "$STUB_LOG"
exec /bin/ln "$@"
EOF

cat > "$STUB_BIN/lipo" <<'EOF'
#!/bin/zsh
print -r -- "lipo $*" >> "$STUB_LOG"
print -r -- "${STUB_LIPO_ARCHS:-arm64}"
exit 0
EOF

chmod 755 "$STUB_BIN"/*(N)

reset_state() {
  rm -f -- "$STUB_LOG" "$STUB_DIR/saw_sign" "$STUB_DIR/stage.json"
  : > "$STUB_LOG"
  rm -rf -- "$sandbox/src-tauri/target"
}

run_build() {
  local ec path_use
  reset_state
  rm -f -- "$STUB_DIR/extra" "$STUB_DIR/symlink_helper" "$STUB_DIR/bad_ctl" "$STUB_DIR/plant"
  if [[ -n ${ORBITTY_TEST_EXTRA:-} ]]; then
    print -r -- "$ORBITTY_TEST_EXTRA" > "$STUB_DIR/extra"
  fi
  if [[ -n ${ORBITTY_TEST_SYMLINK_HELPER:-} ]]; then
    print -r -- "$ORBITTY_TEST_SYMLINK_HELPER" > "$STUB_DIR/symlink_helper"
  fi
  if [[ -n ${ORBITTY_TEST_BAD_CTL:-} ]]; then
    print -r -- 1 > "$STUB_DIR/bad_ctl"
  fi
  if [[ -n ${STUB_PLANT:-} ]]; then
    print -r -- "$STUB_PLANT" > "$STUB_DIR/plant"
  fi
  path_use="$STUB_BIN:$PATH"
  if [[ -n ${STUB_NARROW_PATH:-} ]]; then
    path_use="$STUB_BIN:/usr/bin:/bin:/usr/sbin"
  fi
  ec=0
  set +e
  REPLY_OUT="$(PATH="$path_use" "$script" "$@" 2>&1)"
  ec=$?
  set -e
  REPLY_EC=$ec
}

log_has() {
  local needle=$1
  [[ -f $STUB_LOG ]] && grep -F -q -e "$needle" "$STUB_LOG"
}

cargo_logged() {
  log_has "cargo "
}

success_setup() {
  export STUB_PORCELAIN=""
  export STUB_HEAD="$HEAD_OK"
  export STUB_MAIN="$HEAD_OK"
  export STUB_SECURITY_OUT='  1) C0FFEE "Orbitty Local Codesign"'
  export STUB_AUTHORITY="Orbitty Local Codesign"
  export ORBITTY_TEST_EXTRA=""
  unset ORBITTY_SIGN_IDENTITY
  unset ORBITTY_DMG_OUT
  unset STUB_VERIFY_FAIL
  unset STUB_HDIUTIL_VERIFY_FAIL
  unset STUB_BAD_AUTHORITY_FOR
  unset ORBITTY_TEST_SYMLINK_HELPER
  unset ORBITTY_TEST_BAD_CTL
  unset STUB_PLANT
  unset STUB_NARROW_PATH
  unset STUB_LIPO_ARCHS
  unset OPENROUTER_API_KEY
  unset SOME_API_KEY
  unset ORBITTY_LEAK
}

sign_lines() {
  grep -E '^codesign .*--sign ' "$STUB_LOG" || true
}

tree_fingerprint() {
  python3 -c 'import hashlib, os, sys
root = sys.argv[1]
lines = []
for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
    dirnames.sort()
    filenames.sort()
    for name in dirnames:
        p = os.path.join(dirpath, name)
        rel = os.path.relpath(p, root)
        if os.path.islink(p):
            lines.append("L\t%s\t%s" % (rel, os.readlink(p)))
        else:
            lines.append("D\t%s" % rel)
    for name in filenames:
        p = os.path.join(dirpath, name)
        rel = os.path.relpath(p, root)
        if os.path.islink(p):
            lines.append("L\t%s\t%s" % (rel, os.readlink(p)))
        elif os.path.isfile(p):
            h = hashlib.sha256(open(p, "rb").read()).hexdigest()
            lines.append("F\t%s\t%s" % (rel, h))
        else:
            lines.append("?\t%s" % rel)
lines.sort()
sys.stdout.write("\n".join(lines) + "\n")
' "$1"
}

# --- T1 ad-hoc refused ---
export STUB_SECURITY_OUT='  1) ADHOC "-"'
export ORBITTY_SIGN_IDENTITY="-"
run_build --smoke --out "$tmp/t1-out"
unset ORBITTY_SIGN_IDENTITY
export STUB_SECURITY_OUT='  1) C0FFEE "Orbitty Local Codesign"'
if (( REPLY_EC != 0 )) && ! cargo_logged; then
  pass T1
else
  bad T1
  print -r -- "  exit=$REPLY_EC cargo=$(cargo_logged && print yes || print no)" >&2
fi

# --- T2 missing identity, before cargo ---
export STUB_SECURITY_OUT=""
run_build --smoke --out "$tmp/t2-out"
export STUB_SECURITY_OUT='  1) C0FFEE "Orbitty Local Codesign"'
if (( REPLY_EC != 0 )) && ! cargo_logged; then
  pass T2
else
  bad T2
  print -r -- "  exit=$REPLY_EC" >&2
fi

# --- T3 dirty tree / not main / smoke ---
t3_ok=1
export STUB_PORCELAIN=" M scripts/build-tester-dmg.sh"
export STUB_HEAD="$HEAD_OK"
export STUB_MAIN="$HEAD_OK"
run_build --out "$tmp/t3-dirty"
if (( REPLY_EC == 0 )) || cargo_logged; then
  t3_ok=0
  print -r -- "  T3 dirty did not fail before cargo (exit=$REPLY_EC)" >&2
fi

export STUB_PORCELAIN=""
export STUB_HEAD="bbbbbbb222222222222222222222222222222222"
export STUB_MAIN="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
run_build --out "$tmp/t3-not-main"
if (( REPLY_EC == 0 )) || cargo_logged; then
  t3_ok=0
  print -r -- "  T3 not-main did not fail before cargo (exit=$REPLY_EC)" >&2
fi

export ORBITTY_TEST_EXTRA=""
run_build --smoke --out "$tmp/t3-smoke"
smoke_dmg="$tmp/t3-smoke/Orbitty-0.1.0-alpha-bbbbbbb-arm64-SMOKE.dmg"
if (( REPLY_EC != 0 )) || ! cargo_logged || [[ ! -f $smoke_dmg ]]; then
  t3_ok=0
  print -r -- "  T3 smoke did not pass preflight (exit=$REPLY_EC dmg=$([[ -f $smoke_dmg ]] && print yes || print no))" >&2
  print -r -- "$REPLY_OUT" >&2
fi
export STUB_HEAD="$HEAD_OK"
export STUB_MAIN="$HEAD_OK"
if (( t3_ok )); then
  pass T3
else
  bad T3
fi

# --- T4 bundled relay removed before first codesign; no relay link in target ---
success_setup
run_build --out "$tmp/t4-out"
relay_ln=0
if grep -E '^ln ' "$STUB_LOG" | grep -F -q 'orbitty_relay'; then
  relay_ln=1
fi
relay_left=0
if [[ -d $sandbox/src-tauri/target ]] && find "$sandbox/src-tauri/target" -name 'orbitty_relay' -print -quit | grep -q .; then
  relay_left=1
fi
if (( REPLY_EC == 0 )) && log_has "RELAY_ABSENT" && ! log_has "RELAY_PRESENT" && (( relay_ln == 0 )) && (( relay_left == 0 )); then
  if [[ -f $STUB_DIR/stage.json ]]; then
    macos_has_relay=$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print("orbitty_relay" in d.get("macos", []))' "$STUB_DIR/stage.json")
    if [[ $macos_has_relay == False ]]; then
      pass T4
    else
      bad T4
      print -r -- "  staged app still has orbitty_relay" >&2
    fi
  else
    bad T4
    print -r -- "  no stage snapshot" >&2
  fi
else
  bad T4
  print -r -- "  exit=$REPLY_EC relay_ln=$relay_ln relay_left=$relay_left" >&2
  cat "$STUB_LOG" >&2
fi

# --- T5 helpers then bundle; no --deep on a signing call ---
success_setup
run_build --out "$tmp/t5-out"
t5_ok=1
if (( REPLY_EC != 0 )); then
  t5_ok=0
fi
sign_file="$tmp/t5-signs"
sign_lines > "$sign_file"
deep_sign=0
if grep -E -q -- '--deep' "$sign_file"; then
  deep_sign=1
  t5_ok=0
fi
got_order=$(python3 -c 'import os,sys
lines=open(sys.argv[1], encoding="utf-8").read().splitlines()
names=[]
for line in lines:
    target=line.split()[-1]
    base=os.path.basename(target)
    if base.endswith(".dmg"):
        continue
    names.append(base)
print(" ".join(names))
' "$sign_file")
if [[ $got_order != "orbitty-mcp orbitty-relay orbitty-tabbity-host orbittyctl Orbitty.app" ]]; then
  t5_ok=0
fi
if (( t5_ok )); then
  pass T5
else
  bad T5
  print -r -- "  exit=$REPLY_EC deep_sign=$deep_sign order=[$got_order]" >&2
fi

# --- T6 unexpected helper ---
success_setup
export ORBITTY_TEST_EXTRA="rogue-helper"
run_build --out "$tmp/t6-out"
export ORBITTY_TEST_EXTRA=""
if (( REPLY_EC != 0 )) && [[ $REPLY_OUT == *rogue-helper* ]]; then
  pass T6
else
  bad T6
  print -r -- "  exit=$REPLY_EC" >&2
  print -r -- "$REPLY_OUT" >&2
fi

# --- T7 DMG contents + dmg signed ---
success_setup
run_build --out "$tmp/t7-out"
t7_ok=1
expect_dmg="$tmp/t7-out/Orbitty-0.1.0-alpha-abcdef1-arm64.dmg"
if (( REPLY_EC != 0 )) || [[ ! -s $expect_dmg ]]; then
  t7_ok=0
fi
dmg_signed=$(grep -E '^codesign .*--sign ' "$STUB_LOG" | grep -F -c -- ".dmg" || true)
if (( dmg_signed != 1 )); then
  t7_ok=0
  print -r -- "  dmg sign count=$dmg_signed" >&2
fi
if [[ ! -f $STUB_DIR/stage.json ]]; then
  t7_ok=0
else
  t7_check=$(python3 -c 'import json,sys
d=json.load(open(sys.argv[1], encoding="utf-8"))
names={e["name"]: e for e in d["entries"]}
ok=True
ok = ok and names.get("Orbitty.app", {}).get("dir") is True
ok = ok and names.get("Applications", {}).get("symlink") == "/Applications"
ok = ok and "guide" in d and "Open Anyway" in d["guide"] and "xattr -dr com.apple.quarantine" in d["guide"]
guide=d.get("guide","")
xattr_at=guide.find("xattr -dr com.apple.quarantine")
ax_at=guide.find("asks for Accessibility")
ok = ok and xattr_at >= 0 and ax_at >= 0 and xattr_at < ax_at
ok = ok and "Before ejecting" in guide and "quit Orbitty first" in guide
ok = ok and "companion in spirit form only" in guide
info=d.get("info","")
ok = ok and "version: 0.1.0" in info and "abcdef1234567890abcdef1234567890abcdef12" in info
ok = ok and "identity: Orbitty Local Codesign" in info and "alpha, companion in spirit form only (no faces)" in info
ok = ok and "bundle id: space.orbitty.alpha" in info
ok = ok and "data dir: ~/Library/Application Support/space.orbitty.alpha" in info
ok = ok and "architectures: Apple silicon Macs (M1 or newer) only" in info
ok = ok and "not notarized" in info
ok = ok and "macos minimum: 15.0" in info
ok = ok and d.get("macos") == ["orbitty", "orbitty-mcp", "orbitty-relay", "orbitty-tabbity-host", "orbittyctl"]
ok = ok and d.get("plist_id") == "space.orbitty.alpha"
ok = ok and d.get("plist_commit") == "abcdef1234567890abcdef1234567890abcdef12"
ok = ok and d.get("plist_dirty") is False
print("yes" if ok else "no")
' "$STUB_DIR/stage.json")
  if [[ $t7_check != yes ]]; then
    t7_ok=0
    print -r -- "  stage snapshot mismatch" >&2
    cat "$STUB_DIR/stage.json" >&2
  fi
fi
guide_repo="$(<"$root/docs/TESTER_OPEN_ANYWAY.md")"
if [[ -f $STUB_DIR/stage.json ]]; then
  guide_snap=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8")).get("guide",""), end="")' "$STUB_DIR/stage.json")
  if [[ $guide_snap != "$guide_repo" ]]; then
    t7_ok=0
    print -r -- "  staged guide is not the doc" >&2
  fi
fi
if (( t7_ok )); then
  pass T7
else
  bad T7
  print -r -- "  exit=$REPLY_EC" >&2
  print -r -- "$REPLY_OUT" >&2
fi

# --- T8 dry-run writes nothing; only git and security stubs ---
success_setup
marker_dir="$tmp/t8-out"
mkdir -p "$marker_dir"
print -r -- "keep-me" > "$marker_dir/marker"
before=$(cksum "$marker_dir/marker")
run_build --dry-run --out "$marker_dir"
after=$(cksum "$marker_dir/marker")
others=$(find "$marker_dir" -mindepth 1 ! -name marker | wc -l | tr -d ' ')
called_build=0
if grep -E -q '^(cargo|npm|codesign|hdiutil|ln) ' "$STUB_LOG"; then
  called_build=1
fi
called_pre=0
if grep -E -q '^git ' "$STUB_LOG" && grep -E -q '^security ' "$STUB_LOG"; then
  called_pre=1
fi
if (( REPLY_EC == 0 )) && [[ $before == "$after" ]] && (( others == 0 )) && (( called_build == 0 )) && (( called_pre == 1 )); then
  if [[ $REPLY_OUT == *dry-run:\ writes\ nothing* && $REPLY_OUT == *cargo\ build\ --offline* ]]; then
    pass T8
  else
    bad T8
    print -r -- "  dry-run did not print the plan" >&2
  fi
else
  bad T8
  print -r -- "  exit=$REPLY_EC others=$others called_build=$called_build called_pre=$called_pre" >&2
  print -r -- "$REPLY_OUT" >&2
  cat "$STUB_LOG" >&2
fi

# --- T9 dry-run leaves the sandbox repo byte-identical ---
success_setup
reset_state
fp_before="$(tree_fingerprint "$sandbox")"
run_build --dry-run --out "$tmp/t9-out"
fp_after="$(tree_fingerprint "$sandbox")"
if (( REPLY_EC == 0 )) && [[ $fp_before == "$fp_after" ]]; then
  pass T9
else
  bad T9
  print -r -- "  exit=$REPLY_EC" >&2
  if [[ $fp_before != "$fp_after" ]]; then
    print -r -- "  sandbox tree changed" >&2
  fi
fi

# --- T10 wrong Authority on one helper fails ---
success_setup
export STUB_BAD_AUTHORITY_FOR="orbitty-relay"
run_build --out "$tmp/t10-out"
unset STUB_BAD_AUTHORITY_FOR
if (( REPLY_EC != 0 )); then
  pass T10
else
  bad T10
  print -r -- "  exit=$REPLY_EC" >&2
fi

# --- T11 verify is logged; a failing verify fails the script ---
success_setup
run_build --out "$tmp/t11-ok"
t11_ok=1
if (( REPLY_EC != 0 )) || ! log_has "codesign --verify --deep --strict"; then
  t11_ok=0
  print -r -- "  verify call missing (exit=$REPLY_EC)" >&2
fi
export STUB_VERIFY_FAIL=1
run_build --out "$tmp/t11-fail"
unset STUB_VERIFY_FAIL
if (( REPLY_EC == 0 )); then
  t11_ok=0
  print -r -- "  failing verify did not fail the script" >&2
fi
if (( t11_ok )); then
  pass T11
else
  bad T11
fi

# --- T12 hdiutil verify is logged; a failing verify fails the script ---
success_setup
run_build --out "$tmp/t12-ok"
t12_ok=1
if (( REPLY_EC != 0 )) || ! log_has "hdiutil verify"; then
  t12_ok=0
  print -r -- "  hdiutil verify missing (exit=$REPLY_EC)" >&2
fi
export STUB_HDIUTIL_VERIFY_FAIL=1
run_build --out "$tmp/t12-fail"
unset STUB_HDIUTIL_VERIFY_FAIL
if (( REPLY_EC == 0 )); then
  t12_ok=0
  print -r -- "  failing hdiutil verify did not fail the script" >&2
fi
if (( t12_ok )); then
  pass T12
else
  bad T12
fi

# --- T13 --force guard ---
success_setup
t13_dir="$tmp/t13-out"
mkdir -p "$t13_dir"
t13_dmg="$t13_dir/Orbitty-0.1.0-alpha-abcdef1-arm64.dmg"
print -r -- "ORIGINAL" > "$t13_dmg"
t13_before=$(cksum "$t13_dmg")
run_build --out "$t13_dir"
t13_after=$(cksum "$t13_dmg")
t13_ok=1
if (( REPLY_EC == 0 )) || [[ $t13_before != "$t13_after" ]] || cargo_logged; then
  t13_ok=0
  print -r -- "  without --force exit=$REPLY_EC cargo=$(cargo_logged && print yes || print no)" >&2
fi
run_build --force --out "$t13_dir"
t13_forced=$(<"$t13_dmg")
if (( REPLY_EC != 0 )) || [[ $t13_forced != fake-dmg ]]; then
  t13_ok=0
  print -r -- "  with --force exit=$REPLY_EC body=[$t13_forced]" >&2
fi
if (( t13_ok )); then
  pass T13
else
  bad T13
fi

# --- T14 a helper symlink fails and names it ---
success_setup
export ORBITTY_TEST_SYMLINK_HELPER="orbitty-mcp"
run_build --out "$tmp/t14-out"
unset ORBITTY_TEST_SYMLINK_HELPER
if (( REPLY_EC != 0 )) && [[ $REPLY_OUT == *orbitty-mcp*is\ a\ symlink* ]]; then
  pass T14
else
  bad T14
  print -r -- "  exit=$REPLY_EC" >&2
  print -r -- "$REPLY_OUT" >&2
fi

# --- T15 a failing bundled-ctl check fails before any signing ---
success_setup
export ORBITTY_TEST_BAD_CTL=1
run_build --out "$tmp/t15-out"
unset ORBITTY_TEST_BAD_CTL
signed=0
if grep -E -q '^codesign .*--sign ' "$STUB_LOG"; then
  signed=1
fi
if (( REPLY_EC != 0 )) && (( signed == 0 )); then
  pass T15
else
  bad T15
  print -r -- "  exit=$REPLY_EC signed=$signed" >&2
  print -r -- "$REPLY_OUT" >&2
fi

# --- T16 --out / ORBITTY_DMG_OUT inside the repo, including --dry-run ---
success_setup
t16_ok=1
run_build --dry-run --smoke --out "$sandbox/inside-dry"
if (( REPLY_EC == 0 )) || cargo_logged; then
  t16_ok=0
  print -r -- "  dry-run --out inside did not refuse (exit=$REPLY_EC)" >&2
fi
run_build --smoke --out "$sandbox/inside-real"
if (( REPLY_EC == 0 )) || cargo_logged; then
  t16_ok=0
  print -r -- "  --out inside did not refuse before cargo (exit=$REPLY_EC)" >&2
fi
export ORBITTY_DMG_OUT="$sandbox/env-inside"
run_build --dry-run --smoke
unset ORBITTY_DMG_OUT
if (( REPLY_EC == 0 )) || cargo_logged; then
  t16_ok=0
  print -r -- "  ORBITTY_DMG_OUT inside did not refuse (exit=$REPLY_EC)" >&2
fi
mkdir -p -- "$sandbox/through"
ln -s "$sandbox/through" "$tmp/out-via-link"
run_build --dry-run --smoke --out "$tmp/out-via-link"
if (( REPLY_EC == 0 )) || cargo_logged; then
  t16_ok=0
  print -r -- "  symlink --out into the repo did not refuse (exit=$REPLY_EC)" >&2
fi
ln -s "$sandbox/missing-out" "$tmp/out-dangling"
run_build --dry-run --smoke --out "$tmp/out-dangling"
if (( REPLY_EC == 0 )) || cargo_logged; then
  t16_ok=0
  print -r -- "  dangling symlink --out into the repo did not refuse (exit=$REPLY_EC)" >&2
fi
if (( t16_ok )); then
  pass T16
else
  bad T16
  print -r -- "$REPLY_OUT" >&2
fi

# --- T17 alpha config on the build line; plist id; no dev-thin ---
success_setup
export OPENROUTER_API_KEY="sk-or-v1-SHOULDNOTLEAKSHOULDNOT"
export SOME_API_KEY="leak"
export ORBITTY_LEAK="1"
run_build --out "$tmp/t17-out"
unset OPENROUTER_API_KEY SOME_API_KEY ORBITTY_LEAK
t17_ok=1
if (( REPLY_EC != 0 )); then
  t17_ok=0
fi
if ! log_has "npm --offline run tauri build -- --config "; then
  t17_ok=0
  print -r -- "  T17 missing --config" >&2
fi
if ! log_has "--features alpha"; then
  t17_ok=0
  print -r -- "  T17 missing --features alpha" >&2
fi
if log_has "dev-thin"; then
  t17_ok=0
  print -r -- "  T17 dev-thin was passed" >&2
fi
if ! log_has "KEY_UNSET" || ! log_has "ORBITTY_UNSET" || log_has "KEY_SET" || log_has "ORBITTY_SET"; then
  t17_ok=0
  print -r -- "  T17 build env was not scrubbed" >&2
fi
if [[ ! -f $STUB_DIR/stage.json ]]; then
  t17_ok=0
else
  t17_plist=$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d.get("plist_id"), d.get("plist_commit"), d.get("plist_dirty"))' "$STUB_DIR/stage.json")
  if [[ $t17_plist != "space.orbitty.alpha abcdef1234567890abcdef1234567890abcdef12 False" ]]; then
    t17_ok=0
    print -r -- "  T17 plist fields: [$t17_plist]" >&2
  fi
fi
if (( t17_ok )); then
  pass T17
else
  bad T17
  print -r -- "$REPLY_OUT" >&2
fi

# --- T18 refuse without alpha unless --smoke ---
success_setup
cat > "$sandbox/src-tauri/Cargo.toml" <<'EOF'
[features]
dev-thin = []
EOF
run_build --out "$tmp/t18-out"
t18_ok=1
if (( REPLY_EC == 0 )) || cargo_logged; then
  t18_ok=0
  print -r -- "  T18 non-smoke without alpha did not refuse (exit=$REPLY_EC)" >&2
fi
run_build --smoke --out "$tmp/t18-smoke"
if (( REPLY_EC != 0 )) || ! cargo_logged; then
  t18_ok=0
  print -r -- "  T18 smoke without alpha did not build (exit=$REPLY_EC)" >&2
  print -r -- "$REPLY_OUT" >&2
fi
cat > "$sandbox/src-tauri/Cargo.toml" <<'EOF'
[features]
alpha = []
dev-thin = []
EOF
if (( t18_ok )); then
  pass T18
else
  bad T18
fi

# --- T19 tabbity-host is signed (glob, not the old three names) ---
success_setup
run_build --out "$tmp/t19-out"
if (( REPLY_EC == 0 )) && log_has "Contents/MacOS/orbitty-tabbity-host"; then
  pass T19
else
  bad T19
  print -r -- "  exit=$REPLY_EC" >&2
  print -r -- "$REPLY_OUT" >&2
fi

# --- T20 oxipng missing refuses a non-smoke build ---
success_setup
export STUB_NARROW_PATH=1
run_build --out "$tmp/t20-out"
t20_ok=1
if (( REPLY_EC == 0 )) || cargo_logged; then
  t20_ok=0
  print -r -- "  T20 non-smoke with oxipng missing did not refuse (exit=$REPLY_EC)" >&2
fi
run_build --smoke --out "$tmp/t20-smoke"
if (( REPLY_EC != 0 )) || ! cargo_logged; then
  t20_ok=0
  print -r -- "  T20 smoke should still build without oxipng (exit=$REPLY_EC)" >&2
  print -r -- "$REPLY_OUT" >&2
fi
unset STUB_NARROW_PATH
if (( t20_ok )); then
  pass T20
else
  bad T20
fi

# --- T21 planted secret names and a key-shaped string are refused ---
success_setup
t21_ok=1
for plant in env key secret; do
  export STUB_PLANT="$plant"
  run_build --out "$tmp/t21-$plant"
  signed=0
  if grep -E -q '^codesign .*--sign ' "$STUB_LOG"; then
    signed=1
  fi
  if (( REPLY_EC == 0 )) || cargo_logged && (( signed == 1 )); then
    :
  fi
  if (( REPLY_EC == 0 )) || (( signed != 0 )); then
    t21_ok=0
    print -r -- "  T21 $plant exit=$REPLY_EC signed=$signed" >&2
    print -r -- "$REPLY_OUT" >&2
  fi
done
unset STUB_PLANT
if [[ $REPLY_OUT == *TESTTESTTESTTESTTEST* ]]; then
  t21_ok=0
  print -r -- "  T21 printed the planted key body" >&2
fi
if [[ $REPLY_OUT != *prefix=sk-or-v1-* ]]; then
  t21_ok=0
  print -r -- "  T21 did not print the key prefix" >&2
fi
if (( t21_ok )); then
  pass T21
else
  bad T21
fi

# --- T22 universal passes --target and lipo-checks each MacOS file ---
success_setup
export STUB_LIPO_ARCHS="x86_64 arm64"
run_build --arch universal --out "$tmp/t22-out"
t22_ok=1
uni="$tmp/t22-out/Orbitty-0.1.0-alpha-abcdef1-universal.dmg"
if (( REPLY_EC != 0 )) || [[ ! -f $uni ]]; then
  t22_ok=0
  print -r -- "  T22 exit=$REPLY_EC" >&2
  print -r -- "$REPLY_OUT" >&2
fi
if ! log_has "--target universal-apple-darwin"; then
  t22_ok=0
  print -r -- "  T22 missing --target universal-apple-darwin" >&2
fi
lipo_n=$(grep -c '^lipo ' "$STUB_LOG" || true)
if (( lipo_n != 5 )); then
  t22_ok=0
  print -r -- "  T22 lipo calls=$lipo_n want 5" >&2
fi
if [[ -f $STUB_DIR/stage.json ]]; then
  if ! python3 -c 'import json,sys; info=json.load(open(sys.argv[1])).get("info",""); raise SystemExit(0 if "architectures: x86_64 arm64" in info else 1)' "$STUB_DIR/stage.json"; then
    t22_ok=0
    print -r -- "  T22 BUILD_INFO missing universal line" >&2
  fi
else
  t22_ok=0
fi
export STUB_LIPO_ARCHS="arm64"
run_build --arch universal --out "$tmp/t22-bad"
if (( REPLY_EC == 0 )); then
  t22_ok=0
  print -r -- "  T22 thin binary was accepted as universal" >&2
fi
unset STUB_LIPO_ARCHS
if (( t22_ok )); then
  pass T22
else
  bad T22
fi

# --- T23 dirty tree: non-smoke refused, smoke allowed ---
success_setup
export STUB_PORCELAIN=" M scripts/build-tester-dmg.sh"
run_build --out "$tmp/t23-dirty"
t23_ok=1
if (( REPLY_EC == 0 )) || cargo_logged; then
  t23_ok=0
  print -r -- "  T23 dirty non-smoke did not refuse" >&2
fi
run_build --smoke --out "$tmp/t23-smoke"
if (( REPLY_EC != 0 )) || ! cargo_logged; then
  t23_ok=0
  print -r -- "  T23 dirty smoke did not build (exit=$REPLY_EC)" >&2
  print -r -- "$REPLY_OUT" >&2
fi
export STUB_PORCELAIN=""
if (( t23_ok )); then
  pass T23
else
  bad T23
fi

# --- T24 /Applications and ~/Applications refused, including --dry-run ---
success_setup
t24_ok=1
run_build --dry-run --smoke --out "/Applications/orbitty-tester-nope"
if (( REPLY_EC == 0 )) || cargo_logged; then
  t24_ok=0
  print -r -- "  T24 /Applications did not refuse (exit=$REPLY_EC)" >&2
fi
run_build --dry-run --smoke --out "$HOME/Applications/orbitty-tester-nope"
if (( REPLY_EC == 0 )) || cargo_logged; then
  t24_ok=0
  print -r -- "  T24 ~/Applications did not refuse (exit=$REPLY_EC)" >&2
fi
if [[ -e /Applications/orbitty-tester-nope || -e "$HOME/Applications/orbitty-tester-nope" ]]; then
  t24_ok=0
  print -r -- "  T24 created a refused destination" >&2
fi
if (( t24_ok )); then
  pass T24
else
  bad T24
  print -r -- "$REPLY_OUT" >&2
fi

exit $fail
