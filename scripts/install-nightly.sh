#!/bin/zsh
# Orbitty Nightly — dogfood builds that MUST NOT overwrite the daily-driver
# ~/Applications/Orbitty.app (stable AX path). Same codesign identity;
# separate path + bundle id; Application Support can share stable worlds
# via symlink for dogfood continuity.
set -euo pipefail

cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

APP_NAME="Orbitty Nightly.app"
SRC="src-tauri/target/release/bundle/macos/Orbitty.app"
DEST="$HOME/Applications/$APP_NAME"
BUNDLE_ID="space.orbitty.mac.nightly"
STABLE_SUPPORT="$HOME/Library/Application Support/space.orbitty.mac"
NIGHTLY_SUPPORT="$HOME/Library/Application Support/$BUNDLE_ID"
source "$PWD/scripts/install_lib.sh"
IDENTITY="$(resolve_sign_identity)"

IDS="$(security find-identity -v -p codesigning)" || true
if ! grep -q "$IDENTITY" <<< "$IDS"; then
  echo "ERROR: \"$IDENTITY\" not found — refusing nightly install." >&2
  exit 1
fi

echo "==> Cleaning build-tree bundle"
clean_build_bundle "$PWD"
echo "==> Building orbittyctl (before tauri build)"
(cd src-tauri && cargo build --release --bin orbittyctl)
echo "==> Building orbitty-mcp (before tauri build)"
(cd src-tauri && cargo build --release --bin orbitty-mcp)
echo "==> Building release bundle (nightly)"
mkdir -p src-tauri/target/release
ln -sfn orbitty-relay src-tauri/target/release/orbitty_relay
(cd src-tauri && cargo build --release --bin orbitty-relay) || true
# dev-thin: Matt's THIN stamp (⌘⌥T) is Nightly-only; install-stable.sh builds without it.
npm run tauri build -- --features dev-thin || echo "WARNING: tauri build failed (dmg?) — continuing if .app exists" >&2

if [[ ! -d "$SRC" ]]; then
  echo "error: expected bundle at $SRC" >&2
  exit 1
fi

codesign --force --deep --sign "$IDENTITY" --timestamp=none "$SRC"

echo "==> Installing to $DEST (daily Orbitty.app untouched)"
mkdir -p "$HOME/Applications"
# First install: copy. Later: ditto merge in place for AX path stability.
if [[ ! -d "$DEST" ]]; then
  cp -R "$SRC" "$DEST"
else
  ditto "$SRC" "$DEST"
fi

# Distinct identity so Launch Services doesn't confuse it with daily.
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $BUNDLE_ID" "$DEST/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName Orbitty Nightly" "$DEST/Contents/Info.plist" 2>/dev/null \
  || /usr/libexec/PlistBuddy -c "Add :CFBundleDisplayName string Orbitty Nightly" "$DEST/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleName Orbitty Nightly" "$DEST/Contents/Info.plist" 2>/dev/null || true

# Dock icon: bundled tauri icns was historically RGB-flattened (white halo).
# Prefer brand/Orbitty.iconset (alpha) into Nightly only — never touch daily.
ICONSET="$PWD/brand/Orbitty.iconset"
ICNS_OUT="$DEST/Contents/Resources/icon.icns"
if [[ -d "$ICONSET" ]]; then
  echo "==> Applying transparent Dock icon to Nightly only"
  iconutil -c icns "$ICONSET" -o "$ICNS_OUT"
fi

# GO-ORBITTY-RENAME-MOVE: ditto merge leaves the pre-rename executable behind.
# Drop this line once both installed apps are clean.

codesign --force --deep --sign "$IDENTITY" --timestamp=none "$DEST"
codesign --verify --deep --strict --verbose=2 "$DEST"
codesign -dvv "$DEST" 2>&1 | grep -E '^(Identifier|Authority)' || true

# Share stable worlds/kanban for dogfood (one-time symlink if absent).
if [[ -d "$STABLE_SUPPORT" && ! -e "$NIGHTLY_SUPPORT" ]]; then
  echo "==> Linking nightly Application Support → stable (shared dogfood state)"
  ln -s "$STABLE_SUPPORT" "$NIGHTLY_SUPPORT"
elif [[ -d "$STABLE_SUPPORT" && -d "$NIGHTLY_SUPPORT" && ! -L "$NIGHTLY_SUPPORT" ]]; then
  echo "==> Nightly support dir exists as real folder (not linked) — leaving alone"
fi

# BEGIN mouth-sidecar-config (GO-ORBITTY-DEMATT-RUNTIME-FIX-R2)
if [[ -f "$HOME/Library/LaunchAgents/space.orbitty.mouth.plist" ]]; then
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
    if ! printf '%s\n' '{"url":"http://127.0.0.1:13121"}' > "$tmp"; then
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
fi
# END mouth-sidecar-config

warn_stale_path_ctl "$DEST/Contents/MacOS/orbittyctl"
echo "a PATH orbittyctl without --sock drives Stable, not Nightly"

echo ""
echo "Done. Dogfood THIS app only:"
echo "  1. Quit daily Orbitty (tray → Quit) if both would fight."
echo "  2. open \"$DEST\""
echo "  3. Grant Accessibility for Orbitty Nightly once if prompted."
echo "  Daily-driver remains: ~/Applications/Orbitty.app (untouched)."
