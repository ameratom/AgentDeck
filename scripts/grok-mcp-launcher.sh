#!/usr/bin/env bash
set -euo pipefail

GROK_MCP_DIR="${GROK_MCP_DIR:-$HOME/Grok-MCP}"
KEYCHAIN_SERVICE="com.agentdeck.desktop.provider"
KEYCHAIN_ACCOUNT="xai"

if [[ -z "${XAI_API_KEY:-}" ]]; then
  XAI_API_KEY="$(security find-generic-password -s "$KEYCHAIN_SERVICE" -a "$KEYCHAIN_ACCOUNT" -w 2>/dev/null || true)"
  if [[ -z "$XAI_API_KEY" && -f "$GROK_MCP_DIR/.env" ]]; then
    # shellcheck disable=SC1091
    set -a
    source "$GROK_MCP_DIR/.env"
    set +a
  fi
fi

if [[ -z "${XAI_API_KEY:-}" ]]; then
  echo "Grok MCP: missing XAI_API_KEY. Save it in AgentDeck onboarding or $GROK_MCP_DIR/.env" >&2
  exit 1
fi

export XAI_API_KEY
exec uv run --directory "$GROK_MCP_DIR" python main.py