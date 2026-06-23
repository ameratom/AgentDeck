#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLIST_SRC="${CLOUDFLARED_PLIST_SRC:-$ROOT_DIR/scripts/com.cloudflare.cloudflared.launchagent.plist}"
PLIST_DST="$HOME/Library/LaunchAgents/com.cloudflare.cloudflared.plist"
CLOUDFLARED_CONFIG="${CLOUDFLARED_CONFIG:-$HOME/.cloudflared/config.yml}"
CLOUDFLARED_BIN="${CLOUDFLARED_BIN:-/opt/homebrew/bin/cloudflared}"
LABEL="com.cloudflare.cloudflared"
USER_UID="$(id -u)"

if [[ ! -f "$PLIST_SRC" ]]; then
  echo "Missing plist template: $PLIST_SRC" >&2
  exit 1
fi

if [[ ! -x "$CLOUDFLARED_BIN" ]]; then
  echo "cloudflared not found at $CLOUDFLARED_BIN" >&2
  exit 1
fi

if [[ ! -f "$CLOUDFLARED_CONFIG" ]]; then
  echo "Missing Cloudflare config: $CLOUDFLARED_CONFIG" >&2
  exit 1
fi

mkdir -p "$HOME/Library/LaunchAgents" "$HOME/Library/Logs"

pkill -f "cloudflared tunnel --config $CLOUDFLARED_CONFIG run" 2>/dev/null || true

install -m 644 "$PLIST_SRC" "$PLIST_DST"

launchctl bootout "gui/$USER_UID/$LABEL" 2>/dev/null || true
launchctl bootstrap "gui/$USER_UID" "$PLIST_DST"
launchctl kickstart -k "gui/$USER_UID/$LABEL"

echo "Installed $PLIST_DST"
echo "Logs: $HOME/Library/Logs/com.cloudflare.cloudflared.{out,err}.log"
echo "Verify: cloudflared tunnel info agentdeck-mcp"