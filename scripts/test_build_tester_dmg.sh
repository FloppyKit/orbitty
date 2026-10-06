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
for name in orbitty orbittyctl orbitty-mcp orbitty-relay orbitty_relay; do
  cp "$tmp/ctl" "$fixture/Contents/MacOS/$name"
  chmod 755 "$fixture/Contents/MacOS/$name"
done
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
exit 0
EOF

cat > "$STUB_BIN/npm" <<'EOF'
#!/bin/zsh
print -r -- "npm $*" >> "$STUB_LOG"
dest="src-tauri/target/release/bundle/macos/Orbitty.app"
rm -rf -- "$dest"
mkdir -p -- "${dest:h}"
cp -R -- "$ORBITTY_TEST_FIXTURE" "$dest"
if [[ -n ${ORBITTY_TEST_EXTRA:-} ]]; then
  : > "$dest/Contents/MacOS/$ORBITTY_TEST_EXTRA"
fi
if [[ -n ${ORBITTY_TEST_SYMLINK_HELPER:-} ]]; then
  helper="$dest/Contents/MacOS/$ORBITTY_TEST_SYMLINK_HELPER"
  rm -f -- "$helper"
  ln -s orbitty "$helper"
fi
if [[ -n ${ORBITTY_TEST_BAD_CTL:-} ]]; then
  printf '%s\n' '#!/bin/sh' 'exit 0' > "$dest/Contents/MacOS/orbittyctl"
  chmod 755 "$dest/Contents/MacOS/orbittyctl"
fi
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
snap={"entries": entries, "macos": names}
if os.path.isfile(guide):
    snap["guide"]=open(guide, encoding="utf-8").read()
if os.path.isfile(info):
    snap["info"]=open(info, encoding="utf-8").read()
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

chmod 755 "$STUB_BIN"/*(N)

reset_state() {
  rm -f -- "$STUB_LOG" "$STUB_DIR/saw_sign" "$STUB_DIR/stage.json"
  : > "$STUB_LOG"
  rm -rf -- "$sandbox/src-tauri/target"
}

run_build() {
  local ec
  reset_state
  ec=0
  set +e
  REPLY_OUT="$(PATH="$STUB_BIN:$PATH" "$script" "$@" 2>&1)"
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
smoke_dmg="$tmp/t3-smoke/Orbitty-0.1.0-alpha-bbbbbbb-SMOKE.dmg"
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
if [[ $got_order != "orbittyctl orbitty-mcp orbitty-relay Orbitty.app" ]]; then
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
expect_dmg="$tmp/t7-out/Orbitty-0.1.0-alpha-abcdef1.dmg"
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
ok = ok and d.get("macos") == ["orbitty", "orbitty-mcp", "orbitty-relay", "orbittyctl"]
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
t13_dmg="$t13_dir/Orbitty-0.1.0-alpha-abcdef1.dmg"
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

exit $fail
