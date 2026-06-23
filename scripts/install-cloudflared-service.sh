#!/usr/bin/env bash
set -euo pipefail

# Installs the durable AgentDeck Cloudflare tunnel service.
# Prefers a user LaunchAgent (works with ~/.cloudflared credentials).
# Also stages system config under /Library/Application Support/com.cloudflared
# and disables the legacy token-based LaunchDaemon when run with sudo.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ "$(id -u)" -eq 0 ]]; then
  CLOUDFLARED_CONFIG="${CLOUDFLARED_CONFIG:-/Users/claudemccready/.cloudflared/config.yml}"
  CLOUDFLARED_CRED_DIR="${CLOUDFLARED_CRED_DIR:-/Users/claudemccready/.cloudflared}"
  CLOUDFLARED_TUNNEL_USER="${CLOUDFLARED_TUNNEL_USER:-claudemccready}"
  export CLOUDFLARED_CONFIG CLOUDFLARED_CRED_DIR CLOUDFLARED_TUNNEL_USER
  bash "$ROOT_DIR/scripts/install-cloudflared-launchdaemon.sh" || true
  launchctl bootout system/com.cloudflare.cloudflared 2>/dev/null || true
fi

exec bash "$ROOT_DIR/scripts/install-cloudflared-launchagent.sh"