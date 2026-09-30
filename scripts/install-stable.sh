#!/bin/zsh
# Orbitty — blessed install path (Install C, locked
# 2026-08-21; renamed GO-LS-TAURI-CUTOVER 2026-08-22).
#
# Product install: builds the release bundle, signs it with the stable
# identity (kept on purpose: the macOS Accessibility grant is path+sign,
# so the new path needs one re-grant, then survives rebuilds), and
# installs to ~/Applications/Orbitty.app — the ONLY path to dogfood.
# HARD FAILS if the identity is missing — never ad-hoc: an ad-hoc CDHash
# changes every rebuild, which is how the AX grant died on every install.
#
# Opt-in add-ons (PATH CLIs, relay, mouth sidecar, seat merge) live in
# scripts/install-kitchen.sh. This script does not install them.
set -euo pipefail

cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

APP_NAME="Orbitty.app"
SRC="src-tauri/target/release/bundle/macos/$APP_NAME"
DEST="$HOME/Applications/$APP_NAME"
source "$PWD/scripts/install_lib.sh"
IDENTITY="$(resolve_sign_identity)"

# The stable identity is non-negotiable — check BEFORE the long build.
# `-p codesigning` (not `-p basic`): the codesigning policy is what finds
# this identity from agent shells. Capture-then-grep: a `| grep -q` pipe
# races SIGPIPE under `set -o pipefail` (grep exits on match, security
# dies 141) and false-fails the check.
IDS="$(security find-identity -v -p codesigning)" || true
if ! grep -q "$IDENTITY" <<< "$IDS"; then
  echo "============================================================" >&2
  echo "ERROR: \"$IDENTITY\" not found by:" >&2
  echo "  security find-identity -v -p codesigning" >&2
  echo "Refusing to install — an ad-hoc fallback is how the" >&2
  echo "Accessibility grant broke on every install (an ad-hoc CDHash" >&2
  echo "changes every rebuild)." >&2
  echo "Create a code-signing identity: Keychain Access → Certificate" >&2
  echo "Assistant → Create a Certificate, type Code Signing. Or set" >&2
  echo "ORBITTY_SIGN_IDENTITY to an identity you already have, then" >&2
  echo "re-run install-stable." >&2
  echo "============================================================" >&2
  exit 1
fi

echo "==> Cleaning build-tree bundle"
clean_build_bundle "$PWD"
echo "==> Building orbittyctl (before tauri build)"
(cd src-tauri && cargo build --release --bin orbittyctl)
echo "==> Building orbitty-mcp (before tauri build)"
(cd src-tauri && cargo build --release --bin orbitty-mcp)

echo "==> Building release bundle"
# Tauri externalBin expects underscore name; cargo emits hyphen.
mkdir -p src-tauri/target/release
ln -sfn orbitty-relay src-tauri/target/release/orbitty_relay
# Ensure the hyphen binary exists for the link target (built later too).
(cd src-tauri && cargo build --release --bin orbitty-relay) || true

# The dmg step may fail (hdiutil quirks); the .app is what we install, so a
# dmg failure is not fatal as long as the .app bundle exists.
npm run tauri build || echo "WARNING: tauri build failed (dmg?) — continuing if the .app exists" >&2

if [[ ! -d "$SRC" ]]; then
  echo "error: expected bundle at $SRC" >&2
  exit 1
fi

echo "==> Signing build-tree bundle with \"$IDENTITY\""
codesign --force --deep --sign "$IDENTITY" --timestamp=none "$SRC"

echo "==> Verifying build-tree signature"
codesign --verify --deep --strict --verbose=2 "$SRC"
# -dvv: plain -dv no longer prints the Authority= line (macOS 26).
codesign -dvv "$SRC" 2>&1 | grep -E '^(Identifier|Authority|TeamIdentifier)' || true

echo "==> Installing to $DEST"
mkdir -p "$HOME/Applications"
# Replace IN PLACE (ditto merge), never rm -rf + cp -R: TCC's grant is
# path + stable identity, and keeping the same .app node is what lets one
# human Accessibility grant survive every rebuild.
ditto "$SRC" "$DEST"
# GO-ORBITTY-RENAME-MOVE: ditto merge leaves the pre-rename executable behind.
# Drop this line once both installed apps are clean.

# Sign the INSTALLED app — the blessed path is what TCC sees, so it must
# carry the stable identity itself, not just the build-tree bundle.
echo "==> Signing installed $DEST with \"$IDENTITY\""
codesign --force --deep --sign "$IDENTITY" --timestamp=none "$DEST"

echo "==> Verifying installed signature"
codesign --verify --deep --strict --verbose=2 "$DEST"
# -dvv: plain -dv no longer prints the Authority= line (macOS 26).
codesign -dvv "$DEST" 2>&1 | grep -E '^(Identifier|Authority|TeamIdentifier)' || true
DEST_SIG="$(codesign -dvv "$DEST" 2>&1)" || true
if ! grep -q "^Authority=$IDENTITY$" <<< "$DEST_SIG"; then
  echo "============================================================" >&2
  echo "ERROR: installed app is NOT signed by \"$IDENTITY\"" >&2
  echo "(an ad-hoc Signature= would break Accessibility on the next" >&2
  echo "rebuild). Create a code-signing identity: Keychain Access →" >&2
  echo "Certificate Assistant → Create a Certificate, type Code Signing." >&2
  echo "Or set ORBITTY_SIGN_IDENTITY, then re-run install-stable." >&2
  echo "============================================================" >&2
  exit 1
fi

warn_stale_path_ctl "$DEST/Contents/MacOS/orbittyctl"

cat <<EOF

Done.
  1. Quit any running Orbitty (tray icon → Quit).
  2. Open ONLY: $DEST
       open "$DEST"
  3. Grant Accessibility once for THAT path — one human grant survives
     rebuilds (stable identity + same path is what TCC remembers).
     System Settings → Privacy & Security → Accessibility.
  4. Do not run target/ binaries or \`tauri dev\` for daily use.
  5. The CLI ships inside the app:
       $DEST/Contents/MacOS/orbittyctl
  6. Matt's kitchen add-ons: scripts/install-kitchen.sh --help
EOF
