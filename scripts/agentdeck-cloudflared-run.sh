#!/usr/bin/env bash
set -euo pipefail

export HOME="${AGENTDECK_CLOUDFLARED_HOME:-/Users/claudemccready}"
export USER="${AGENTDECK_CLOUDFLARED_USER:-claudemccready}"
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

exec /opt/homebrew/bin/cloudflared tunnel \
  --config "/Library/Application Support/com.cloudflared/config.yml" \
  run