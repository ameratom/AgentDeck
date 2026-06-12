#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_ENV="$ROOT_DIR/scripts/chatgpt-mcp-tunnel.local.env"

if [[ -f "$LOCAL_ENV" ]]; then
  # shellcheck disable=SC1091
  source "$LOCAL_ENV"
fi

: "${OPENAI_TUNNEL_ID:?Set OPENAI_TUNNEL_ID in scripts/chatgpt-mcp-tunnel.local.env}"
: "${OPENAI_API_KEY:?Set OPENAI_API_KEY in scripts/chatgpt-mcp-tunnel.local.env}"
AGENTDECK_MCP_URL="${AGENTDECK_MCP_URL:-http://127.0.0.1:7823/mcp}"
TUNNEL_CLIENT_BIN="${TUNNEL_CLIENT_BIN:-tunnel-client}"

if ! command -v "$TUNNEL_CLIENT_BIN" >/dev/null 2>&1; then
  echo "Missing tunnel-client. Install OpenAI tunnel-client and add it to PATH," >&2
  echo "or set TUNNEL_CLIENT_BIN in scripts/chatgpt-mcp-tunnel.local.env." >&2
  echo "Docs: https://developers.openai.com/api/docs/guides/secure-mcp-tunnels" >&2
  exit 1
fi

echo "Forwarding tunnel ${OPENAI_TUNNEL_ID} -> ${AGENTDECK_MCP_URL}"
echo "Ensure AgentDeck.app is running before connecting ChatGPT."

exec "$TUNNEL_CLIENT_BIN" \
  --tunnel-id "$OPENAI_TUNNEL_ID" \
  --api-key "$OPENAI_API_KEY" \
  --mcp-url "$AGENTDECK_MCP_URL"