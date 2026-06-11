#!/usr/bin/env bash
set -euo pipefail

GROK_MCP_DIR="${GROK_MCP_DIR:-$HOME/Grok-MCP}"

if [[ -z "${XAI_API_KEY:-}" ]]; then
  if [[ -f "$GROK_MCP_DIR/.env" ]]; then
    # shellcheck disable=SC1091
    set -a
    source "$GROK_MCP_DIR/.env"
    set +a
  fi
fi

if [[ -z "${XAI_API_KEY:-}" ]]; then
  echo "Grok MCP: missing XAI_API_KEY. Set it in the launcher environment or $GROK_MCP_DIR/.env" >&2
  exit 1
fi

export XAI_API_KEY
exec uv run --directory "$GROK_MCP_DIR" python main.py
