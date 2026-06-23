#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLIST_SRC="${CLOUDFLARED_PLIST_SRC:-$ROOT_DIR/scripts/com.cloudflare.cloudflared.plist}"
PLIST_DST="/Library/LaunchDaemons/com.cloudflare.cloudflared.plist"
SOURCE_CONFIG="${CLOUDFLARED_CONFIG:-$HOME/.cloudflared/config.yml}"
SOURCE_CRED_DIR="${CLOUDFLARED_CRED_DIR:-$HOME/.cloudflared}"
DEST_DIR="/Library/Application Support/com.cloudflared"
DEST_CONFIG="$DEST_DIR/config.yml"
CLOUDFLARED_BIN="${CLOUDFLARED_BIN:-/opt/homebrew/bin/cloudflared}"
LABEL="com.cloudflare.cloudflared"
TUNNEL_USER="${CLOUDFLARED_TUNNEL_USER:-$(logname 2>/dev/null || id -un)}"

if [[ "$(id -u)" -ne 0 ]]; then
  echo "Run with sudo: sudo $0" >&2
  exit 1
fi

if [[ ! -f "$PLIST_SRC" ]]; then
  echo "Missing plist template: $PLIST_SRC" >&2
  exit 1
fi

if [[ ! -x "$CLOUDFLARED_BIN" ]]; then
  echo "cloudflared not found at $CLOUDFLARED_BIN" >&2
  exit 1
fi

if [[ ! -f "$SOURCE_CONFIG" ]]; then
  echo "Missing Cloudflare config: $SOURCE_CONFIG" >&2
  exit 1
fi

tunnel_id="$(awk '/^tunnel:/{print $2}' "$SOURCE_CONFIG")"
if [[ -z "$tunnel_id" ]]; then
  echo "Unable to read tunnel id from $SOURCE_CONFIG" >&2
  exit 1
fi

source_cred="$SOURCE_CRED_DIR/${tunnel_id}.json"
if [[ ! -f "$source_cred" ]]; then
  echo "Missing tunnel credentials: $source_cred" >&2
  exit 1
fi

pkill -f "cloudflared tunnel --config $SOURCE_CONFIG run" 2>/dev/null || true
pkill -f "cloudflared tunnel --config $DEST_CONFIG run" 2>/dev/null || true

install -d -m 755 -o root -g wheel "$DEST_DIR"
install -m 400 -o "$TUNNEL_USER" -g staff "$source_cred" "$DEST_DIR/${tunnel_id}.json"

cat >"$DEST_CONFIG" <<EOF
tunnel: ${tunnel_id}
credentials-file: ${DEST_DIR}/${tunnel_id}.json

ingress:
  - hostname: mcp.thedeckisstacked.win
    service: http://127.0.0.1:7823
  - service: http_status:404
EOF
chown root:wheel "$DEST_CONFIG"
chmod 644 "$DEST_CONFIG"

WRAPPER_SRC="${CLOUDFLARED_WRAPPER_SRC:-$ROOT_DIR/scripts/agentdeck-cloudflared-run.sh}"
WRAPPER_DST="/usr/local/bin/agentdeck-cloudflared-run.sh"
if [[ ! -f "$WRAPPER_SRC" ]]; then
  echo "Missing wrapper script: $WRAPPER_SRC" >&2
  exit 1
fi
install -m 755 -o root -g wheel "$WRAPPER_SRC" "$WRAPPER_DST"

install -m 644 -o root -g wheel "$PLIST_SRC" "$PLIST_DST"

launchctl bootout "system/$LABEL" 2>/dev/null || true
launchctl bootstrap system "$PLIST_DST"
launchctl kickstart -k "system/$LABEL"

echo "Installed $PLIST_DST"
echo "Tunnel config: $DEST_CONFIG"
echo "Logs: /Library/Logs/com.cloudflare.cloudflared.{out,err}.log"
echo "Verify: cloudflared tunnel info agentdeck-mcp"