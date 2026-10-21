#!/bin/zsh
# Install the Tabbity native-messaging host for Chrome or Brave.
# Cook runs only --dry-run. Chuck or Matt runs it for real.
# Usage: install-tabbity-host.sh --browser chrome|brave [--dry-run] [--uninstall]
set -euo pipefail

browser=""
dry=0
uninstall=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --browser)
      browser="${2:-}"
      shift 2
      ;;
    --dry-run) dry=1; shift ;;
    --uninstall) uninstall=1; shift ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

case "$browser" in
  chrome)
    host_dir="${HOME}/Library/Application Support/Google/Chrome/NativeMessagingHosts"
    ;;
  brave)
    host_dir="${HOME}/Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts"
    ;;
  *)
    echo "need --browser chrome|brave" >&2
    exit 2
    ;;
esac

name="space.orbitty.tabbity"
manifest="${host_dir}/${name}.json"
ext_id="pbdnjlgnnkgbmnikjnfakajdkndgdicn"
script_dir="${0:A:h}"
repo="${script_dir:h}"

if [[ -n "${ORBITTY_TABBITY_HOST:-}" ]]; then
  bin="${ORBITTY_TABBITY_HOST}"
elif [[ -x "${HOME}/Applications/Orbitty Nightly.app/Contents/MacOS/orbitty-tabbity-host" ]]; then
  bin="${HOME}/Applications/Orbitty Nightly.app/Contents/MacOS/orbitty-tabbity-host"
else
  bin="${repo}/src-tauri/target/release/orbitty-tabbity-host"
fi

if (( uninstall )); then
  if (( dry )); then
    print -r -- "{\"browser\":\"${browser}\",\"uninstall\":true,\"path\":\"${manifest}\"}"
    exit 0
  fi
  rm -f -- "${manifest}"
  echo "removed ${manifest}"
  exit 0
fi

json="$(cat <<EOF
{
  "name": "${name}",
  "description": "Orbitty Tabbity",
  "path": "${bin}",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://${ext_id}/"]
}
EOF
)"

if (( dry )); then
  print -r -- "${json}"
  exit 0
fi

mkdir -p -- "${host_dir}"
tmp="$(mktemp "${host_dir}/.${name}.XXXXXX")"
print -r -- "${json}" > "${tmp}"
mv -f -- "${tmp}" "${manifest}"
echo "wrote ${manifest}"
