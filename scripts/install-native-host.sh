#!/usr/bin/env bash
# Register the HavenKeys native messaging host with installed browsers
# (Linux and macOS, current user only).
#
#   scripts/install-native-host.sh [path/to/havenkeys-native-host]
#   scripts/install-native-host.sh --uninstall
#
# Build the host first: cargo build --release -p havenkeys-native-host
set -euo pipefail

NAME="com.havenkeys.bridge"
CHROME_ORIGIN="chrome-extension://olbclkanfbmilnmfhoojgcnpgdmilfmf/"
FIREFOX_ID="havenkeys@havenkeys.app"

case "$(uname -s)" in
  Linux)
    chromium_dirs=(
      "$HOME/.config/google-chrome"
      "$HOME/.config/google-chrome-beta"
      "$HOME/.config/chromium"
      "$HOME/.config/BraveSoftware/Brave-Browser"
      "$HOME/.config/microsoft-edge"
      "$HOME/.config/vivaldi"
    )
    firefox_dir="$HOME/.mozilla"
    firefox_hosts="$HOME/.mozilla/native-messaging-hosts"
    ;;
  Darwin)
    s="$HOME/Library/Application Support"
    chromium_dirs=(
      "$s/Google/Chrome"
      "$s/Google/Chrome Beta"
      "$s/Chromium"
      "$s/BraveSoftware/Brave-Browser"
      "$s/Microsoft Edge"
      "$s/Vivaldi"
    )
    firefox_dir="$s/Mozilla"
    firefox_hosts="$s/Mozilla/NativeMessagingHosts"
    ;;
  *)
    echo "Unsupported OS. On Windows use scripts/install-native-host.ps1." >&2
    exit 1
    ;;
esac

if [[ "${1:-}" == "--uninstall" ]]; then
  for d in "${chromium_dirs[@]}"; do rm -f "$d/NativeMessagingHosts/$NAME.json"; done
  rm -f "$firefox_hosts/$NAME.json"
  echo "Removed the HavenKeys native host registration."
  exit 0
fi

bin="${1:-target/release/havenkeys-native-host}"
if [[ ! -x "$bin" ]]; then
  echo "Native host not found at $bin." >&2
  echo "Build it with: cargo build --release -p havenkeys-native-host" >&2
  exit 1
fi
bin="$(cd "$(dirname "$bin")" && pwd)/$(basename "$bin")"
# The path is embedded in JSON below; refuse anything that would need escaping.
if [[ "$bin" == *[\"\\]* || "$bin" == *[[:cntrl:]]* ]]; then
  echo "The host path contains a quote, backslash or control character; move it somewhere simpler." >&2
  exit 1
fi

write_manifest() { # $1 = file, $2 = allowed key, $3 = allowed value
  mkdir -p "$(dirname "$1")"
  umask 022
  cat > "$1" <<JSON
{
  "name": "$NAME",
  "description": "HavenKeys desktop bridge",
  "path": "$bin",
  "type": "stdio",
  "$2": ["$3"]
}
JSON
  echo "  $1"
}

echo "Registered $bin for:"
found=0
for d in "${chromium_dirs[@]}"; do
  if [[ -d "$d" ]]; then
    write_manifest "$d/NativeMessagingHosts/$NAME.json" allowed_origins "$CHROME_ORIGIN"
    found=1
  fi
done
if [[ -d "$firefox_dir" ]]; then
  write_manifest "$firefox_hosts/$NAME.json" allowed_extensions "$FIREFOX_ID"
  found=1
fi
if [[ $found -eq 0 ]]; then
  echo "  (no supported browser profile found)" >&2
  exit 1
fi
