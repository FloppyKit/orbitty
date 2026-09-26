#!/bin/zsh
# Orbitty — blessed install path (Install C, locked
# 2026-08-21; renamed GO-LS-TAURI-CUTOVER 2026-08-22).
#
# Builds the release bundle, signs it with the stable "Orbitty Local
# Codesign" identity (kept on purpose: the macOS Accessibility grant is
# path+sign, so the new path needs one re-grant, then survives rebuilds),
# and installs to ~/Applications/Orbitty.app — the ONLY path to dogfood.
# HARD FAILS if the identity is missing — never ad-hoc: an ad-hoc CDHash
# changes every rebuild, which is how the AX grant died on every install.
set -euo pipefail

cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

APP_NAME="Orbitty.app"
SRC="src-tauri/target/release/bundle/macos/$APP_NAME"
DEST="$HOME/Applications/$APP_NAME"
IDENTITY="Orbitty Local Codesign"

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
  echo "changes every rebuild). Nest must re-sign: restore the" >&2
  echo "identity in the login keychain, then re-run install-stable." >&2
  echo "============================================================" >&2
  exit 1
fi

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
rm -f "$DEST/Contents/MacOS/livesprite"

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
  echo "rebuild). Nest must re-sign: fix the keychain identity and" >&2
  echo "re-run install-stable." >&2
  echo "============================================================" >&2
  exit 1
fi

# GO-ORBITTY-CONTROL-PLANE-0: the control-socket CLI. tauri build only
# guarantees the app bin, so build orbittyctl explicitly (incremental —
# a no-op when cargo already produced it). Signed with the same identity,
# then installed to the ONE documented dogfood path: ~/.local/bin.
CTL_SRC="src-tauri/target/release/orbittyctl"
CTL_DIR="$HOME/.local/bin"
echo "==> Building orbittyctl"
(cd src-tauri && cargo build --release --bin orbittyctl)
if [[ ! -x "$CTL_SRC" ]]; then
  echo "error: expected orbittyctl at $CTL_SRC" >&2
  exit 1
fi
codesign --force --sign "$IDENTITY" --timestamp=none "$CTL_SRC"
echo "==> Installing orbittyctl to $CTL_DIR"
mkdir -p "$CTL_DIR"
cp "$CTL_SRC" "$CTL_DIR/orbittyctl"

# GO-ORBITTY-MCP-0: thin stdio adapter. Same identity, same PATH dir.
MCP_SRC="src-tauri/target/release/orbitty-mcp"
echo "==> Building orbitty-mcp"
(cd src-tauri && cargo build --release --bin orbitty-mcp)
if [[ ! -x "$MCP_SRC" ]]; then
  echo "error: expected orbitty-mcp at $MCP_SRC" >&2
  exit 1
fi
codesign --force --sign "$IDENTITY" --timestamp=none "$MCP_SRC"
echo "==> Installing orbitty-mcp to $CTL_DIR"
cp "$MCP_SRC" "$CTL_DIR/orbitty-mcp"

# GO-ORBITTY-RELAY-SIDECAR: loopback NIP-01 sidecar. Kitchen LaunchAgent
# only (space.orbitty.mac). Demo profiles do not get this job.
RELAY_SRC="src-tauri/target/release/orbitty-relay"
echo "==> Building orbitty-relay"
(cd src-tauri && cargo build --release --bin orbitty-relay)
if [[ ! -x "$RELAY_SRC" ]]; then
  echo "error: expected orbitty-relay at $RELAY_SRC" >&2
  exit 1
fi
codesign --force --sign "$IDENTITY" --timestamp=none "$RELAY_SRC"
echo "==> Installing orbitty-relay to $CTL_DIR"
cp "$RELAY_SRC" "$CTL_DIR/orbitty-relay"
RELAY_PLIST="$HOME/Library/LaunchAgents/space.orbitty.relay.plist"
RELAY_LOG="$HOME/Library/Application Support/space.orbitty.mac/relay"
mkdir -p "$RELAY_LOG"
cat > "$RELAY_PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>space.orbitty.relay</string>
  <key>ProgramArguments</key>
  <array>
    <string>$CTL_DIR/orbitty-relay</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>$RELAY_LOG/relay.log</string>
  <key>StandardErrorPath</key>
  <string>$RELAY_LOG/relay.err</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>HOME</key>
    <string>$HOME</string>
    <key>PATH</key>
    <string>$CTL_DIR:/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin</string>
  </dict>
</dict>
</plist>
PLIST
UID_NUM="$(id -u)"
launchctl bootout "gui/${UID_NUM}/space.orbitty.relay" >/dev/null 2>&1 || true
if ! launchctl bootstrap "gui/${UID_NUM}" "$RELAY_PLIST"; then
  echo "WARNING: launchctl bootstrap space.orbitty.relay failed — binary is installed" >&2
fi

# GO-ORBITTY-1B-SIDECAR: kitchen venv + loopback HTTP. Weights stay
# on MiniHangar — never ditto into Orbitty.app. Fail open: keywords.
MOUTH_DIR="$PWD/sidecar/mouth"
MOUTH_WEIGHTS="/Volumes/MiniHangar/models/orbitty-mouth/mlx-community/Qwen2.5-1.5B-Instruct-4bit"
if [[ -f "$MOUTH_DIR/setup.sh" ]]; then
  echo "==> Mouth sidecar venv"
  bash "$MOUTH_DIR/setup.sh" || echo "WARNING: mouth sidecar venv failed — Quick ack stays keywords" >&2
fi
if [[ -x "$MOUTH_DIR/.venv/bin/python" ]]; then
  MOUTH_PLIST="$HOME/Library/LaunchAgents/space.orbitty.mouth.plist"
  MOUTH_LOG="$HOME/Library/Application Support/space.orbitty.mac/mouth"
  mkdir -p "$MOUTH_LOG"
  cat > "$MOUTH_PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>space.orbitty.mouth</string>
  <key>WorkingDirectory</key>
  <string>$MOUTH_DIR</string>
  <key>ProgramArguments</key>
  <array>
    <string>$MOUTH_DIR/.venv/bin/python</string>
    <string>$MOUTH_DIR/serve.py</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>$MOUTH_LOG/mouth.log</string>
  <key>StandardErrorPath</key>
  <string>$MOUTH_LOG/mouth.err</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>HOME</key>
    <string>$HOME</string>
    <key>PATH</key>
    <string>$MOUTH_DIR/.venv/bin:$CTL_DIR:/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin</string>
    <key>ORBITTY_MOUTH_PORT</key>
    <string>13121</string>
    <key>ORBITTY_MOUTH_WEIGHTS</key>
    <string>$MOUTH_WEIGHTS</string>
  </dict>
</dict>
</plist>
PLIST
  UID_NUM="$(id -u)"
  launchctl bootout "gui/${UID_NUM}/space.orbitty.mouth" >/dev/null 2>&1 || true
  if ! launchctl bootstrap "gui/${UID_NUM}" "$MOUTH_PLIST"; then
    echo "WARNING: launchctl bootstrap space.orbitty.mouth failed — keywords stay Quick ack" >&2
  fi
else
  echo "WARNING: sidecar/mouth/.venv missing — Quick ack stays keywords" >&2
fi

echo "==> Writing Goose orbitty seat (idempotent merge)"
"$PWD/scripts/write-goose-orbitty-seat.sh" || echo "WARNING: Goose seat write failed — MCP binary is still installed" >&2

cat <<EOF

Done. Dogfood rules:
  1. Quit any running Orbitty (tray icon → Quit).
  2. Open ONLY: $DEST
       open "$DEST"
  3. Grant/verify Accessibility for THAT path — ONE human grant now
     survives rebuilds (stable identity + same path is what TCC
     remembers). System Settings → Privacy & Security → Accessibility.
  4. Do not run target/ binaries or \`tauri dev\` for daily use.
  5. orbittyctl (control-socket CLI) installed to:
       $CTL_DIR/orbittyctl
     PATH recipe (add to ~/.zshrc):
       export PATH="\$HOME/.local/bin:\$PATH"
     Try: orbittyctl status · orbittyctl windows list ·
       orbittyctl --json state get
  6. orbitty-mcp (stdio MCP adapter, same socket) installed to:
       $CTL_DIR/orbitty-mcp
     Goose seat auto-merged into ~/.config/goose/config.yaml
     (idempotent; backup first). Manual fallback:
       docs/ORBITTY_MCP_GOOSE_SNIPPET.md
  7. orbitty-relay (loopback NIP-01 sidecar) installed to:
       $CTL_DIR/orbitty-relay
     LaunchAgent space.orbitty.relay KeepAlive · 127.0.0.1:13120
     Smoke: orbitty-relay smoke
     Optional one-user replica: Settings Replica URL or
       orbittyctl relay replica --url 'ws://100.x:13120'
     Empty URL = local only. Never 0.0.0.0. nsec stays on this Mac.
     One-user VPS replica (Tailscale/SSH, never 0.0.0.0):
       docs/ORBITTY_RELAY_VPS.md
EOF
