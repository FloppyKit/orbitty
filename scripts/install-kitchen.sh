#!/bin/zsh
# Orbitty kitchen add-ons for an already-installed Stable app.
#
# Matt's local setup, opt-in, FOSS and local-only: PATH CLIs, the relay
# LaunchAgent, the mouth sidecar, and the Goose seat. Not part of the
# product install. Nothing under a home buzz directory.
#
# No flags: print usage and write nothing.
# Copies binaries from ~/Applications/Orbitty.app as-is (already signed).
# Re-signing would make warn_stale_path_ctl's cmp always differ.
set -euo pipefail

cd "$(dirname "$0")/.."
source "$PWD/scripts/install_lib.sh"

usage() {
  cat <<'EOF'
usage: scripts/install-kitchen.sh [--cli] [--relay] [--mouth] [--goose] [--all] [--dry-run] [--help]

  --cli      copy orbittyctl, orbitty-mcp, and orbitty-relay into ~/.local/bin
  --relay    relay LaunchAgent (implies --cli)
  --mouth    mouth sidecar venv + LaunchAgent
             weights: ORBITTY_MOUTH_WEIGHTS, else ~/.config/orbitty/mouth-sidecar.json,
             else the existing mouth plist. Empty skips this add-on.
  --goose    Goose orbitty seat (implies --cli)
  --all      all of the above
  --dry-run  print the plan and write nothing
  --help     this text

No flags: print this usage and write nothing.
EOF
}

if [[ $# -eq 0 ]]; then
  usage
  exit 0
fi

do_cli=0
do_relay=0
do_mouth=0
do_goose=0
dry=0
help=0

while [[ $# -gt 0 ]]; do
  case $1 in
    --cli) do_cli=1 ;;
    --relay) do_relay=1; do_cli=1 ;;
    --mouth) do_mouth=1 ;;
    --goose) do_goose=1; do_cli=1 ;;
    --all) do_cli=1; do_relay=1; do_mouth=1; do_goose=1 ;;
    --dry-run) dry=1 ;;
    --help|-h) help=1 ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

if [[ $help == 1 ]]; then
  usage
  exit 0
fi

APP="$HOME/Applications/Orbitty.app"
MACOS="$APP/Contents/MacOS"
CTL_DIR="$HOME/.local/bin"

if [[ $dry == 1 ]]; then
  echo "dry-run: write nothing"
  if [[ $do_cli == 1 ]]; then
    echo "dry-run: --cli copy orbittyctl, orbitty-mcp, orbitty-relay from $APP into ~/.local/bin"
  fi
  if [[ $do_relay == 1 ]]; then
    echo "dry-run: --relay LaunchAgent space.orbitty.relay"
  fi
  if [[ $do_mouth == 1 ]]; then
    echo "dry-run: --mouth sidecar"
  fi
  if [[ $do_goose == 1 ]]; then
    echo "dry-run: --goose seat"
  fi
  exit 0
fi

if [[ ! -d $APP ]]; then
  echo "error: installed app not found at $APP" >&2
  exit 1
fi

if [[ $do_cli == 1 ]]; then
  b=""
  for b in orbittyctl orbitty-mcp orbitty-relay; do
    if [[ ! -x $MACOS/$b ]]; then
      echo "error: expected executable $MACOS/$b" >&2
      exit 1
    fi
  done
  mkdir -p -- "$CTL_DIR"
  for b in orbittyctl orbitty-mcp orbitty-relay; do
    cp -p -- "$MACOS/$b" "$CTL_DIR/$b"
  done
  echo "==> Installed orbittyctl, orbitty-mcp, orbitty-relay to $CTL_DIR"
fi

if [[ $do_relay == 1 ]]; then
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
fi

if [[ $do_mouth == 1 ]]; then
  MOUTH_WEIGHTS="$(kitchen_mouth_weights "$HOME")"
  if [[ -z $MOUTH_WEIGHTS ]]; then
    echo "no weights configured (set ORBITTY_MOUTH_WEIGHTS)"
  else
    MOUTH_DIR="$PWD/sidecar/mouth"
    if [[ -f $MOUTH_DIR/setup.sh ]]; then
      echo "==> Mouth sidecar venv"
      bash "$MOUTH_DIR/setup.sh" || echo "WARNING: mouth sidecar venv failed — Quick ack stays keywords" >&2
    fi
    if [[ -x $MOUTH_DIR/.venv/bin/python ]]; then
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
      else
# BEGIN mouth-sidecar-config-kitchen
if ! (
  cfg="$HOME/.config/orbitty/mouth-sidecar.json"
  dir="$HOME/.config/orbitty"
  if [[ -L $dir ]]; then
    echo "WARNING: mouth sidecar config not written ($dir is a symlink)" >&2
    exit 0
  fi
  if [[ -e $cfg || -L $cfg ]]; then
    exit 0
  fi
  if ! mkdir -p -- "$dir"; then
    exit 1
  fi
  tmp=$(mktemp "$dir/.mouth-sidecar.json.XXXXXX") || exit 1
  if ! chmod 600 "$tmp"; then
    rm -f -- "$tmp"
    exit 1
  fi
  if ! command -v python3 >/dev/null 2>&1 \
    || ! python3 -c 'import json,sys;print(json.dumps({"url":"http://127.0.0.1:13121","weights":sys.argv[1]}))' "${MOUTH_WEIGHTS-}" > "$tmp"; then
    rm -f -- "$tmp"
    exit 1
  fi
  if [[ ! -s $tmp ]]; then
    rm -f -- "$tmp"
    exit 1
  fi
  if ! mv -f -- "$tmp" "$cfg"; then
    rm -f -- "$tmp"
    exit 1
  fi
); then
  echo "WARNING: mouth sidecar config not written (could not create the config file)" >&2
fi
# END mouth-sidecar-config-kitchen
      fi
    else
      echo "WARNING: sidecar/mouth/.venv missing — Quick ack stays keywords" >&2
    fi
  fi
fi

if [[ $do_goose == 1 ]]; then
  echo "==> Writing Goose orbitty seat (idempotent merge)"
  "$PWD/scripts/write-goose-orbitty-seat.sh" || echo "WARNING: Goose seat write failed — MCP binary is still installed" >&2
fi
