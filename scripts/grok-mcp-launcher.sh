#!/usr/bin/env bash
set -euo pipefail

GROK_MCP_DIR="${GROK_MCP_DIR:-$HOME/Grok-MCP}"
BRIDGE_FILE="${AGENTDECK_GROK_MCP_BRIDGE:-$HOME/Library/Application Support/com.agentdeck.desktop/grok-mcp.env}"

if [[ -z "${XAI_API_KEY:-}" ]]; then
  if [[ -f "$GROK_MCP_DIR/.env" ]]; then
    # shellcheck disable=SC1091
    set -a
    source "$GROK_MCP_DIR/.env"
    set +a
  fi
fi

if [[ -z "${XAI_API_KEY:-}" && -f "$BRIDGE_FILE" ]]; then
  # shellcheck disable=SC1091
  set -a
  source "$BRIDGE_FILE"
  set +a
fi

if [[ -z "${XAI_API_KEY:-}" ]]; then
  echo "Grok MCP: missing XAI_API_KEY." >&2
  echo "Set it in AgentDeck (Providers → xAI), in $GROK_MCP_DIR/.env," >&2
  echo "or sync the bridge file at $BRIDGE_FILE." >&2
  exit 1
fi

export XAI_API_KEY
exec uv run --directory "$GROK_MCP_DIR" python main.py