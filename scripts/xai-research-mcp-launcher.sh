#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_SUPPORT_DIR="${HOME}/Library/Application Support/com.agentdeck.desktop"
XAI_ENV="${AGENTDECK_XAI_ENV:-$APP_SUPPORT_DIR/grok-mcp.env}"
NODE_BIN="${NODE_BIN:-$(command -v node || true)}"

if [[ -f "$XAI_ENV" ]]; then
  # shellcheck disable=SC1090
  set -a
  source "$XAI_ENV"
  set +a
fi

: "${XAI_API_KEY:?Save or sync the xAI credential in AgentDeck Providers first.}"
export XAI_API_KEY

if [[ -z "$NODE_BIN" ]]; then
  echo "Node.js is required for agentdeck-xai-research-mcp." >&2
  exit 1
fi

exec "$NODE_BIN" "$ROOT_DIR/scripts/xai-research-mcp.mjs"
