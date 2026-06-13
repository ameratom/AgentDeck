#!/usr/bin/env bash
set -euo pipefail

APP_SUPPORT_DIR="${HOME}/Library/Application Support/com.agentdeck.desktop"
LOCAL_ENV="${AGENTDECK_TUNNEL_ENV:-$APP_SUPPORT_DIR/chatgpt-mcp-tunnel.env}"

if [[ -f "$LOCAL_ENV" ]]; then
  # shellcheck disable=SC1091
  source "$LOCAL_ENV"
fi

: "${OPENAI_TUNNEL_ID:?Set OPENAI_TUNNEL_ID in $LOCAL_ENV}"
: "${OPENAI_API_KEY:?Set OPENAI_API_KEY in $LOCAL_ENV}"
AGENTDECK_MCP_URL="${AGENTDECK_MCP_URL:-http://127.0.0.1:7823/mcp}"
TUNNEL_CLIENT_BIN="${TUNNEL_CLIENT_BIN:-tunnel-client}"

if [[ "$OPENAI_TUNNEL_ID" == *"YOUR_TUNNEL_ID_HERE"* || "$OPENAI_TUNNEL_ID" == "tunnel_..." ]]; then
  echo "Replace the placeholder OPENAI_TUNNEL_ID in $LOCAL_ENV." >&2
  exit 1
fi

if [[ "$OPENAI_API_KEY" == *"YOUR_OPENAI_API_KEY_HERE"* || "$OPENAI_API_KEY" == "sk-..." ]]; then
  echo "Replace the placeholder OPENAI_API_KEY in $LOCAL_ENV." >&2
  exit 1
fi

if ! command -v "$TUNNEL_CLIENT_BIN" >/dev/null 2>&1; then
  echo "Missing tunnel-client. Install OpenAI tunnel-client and add it to PATH," >&2
  echo "or set TUNNEL_CLIENT_BIN in $LOCAL_ENV." >&2
  echo "Docs: https://developers.openai.com/api/docs/guides/secure-mcp-tunnels" >&2
  exit 1
fi

if ! curl -sS --max-time 1 "$AGENTDECK_MCP_URL" >/dev/null 2>&1; then
  OPEN_ARGS=(-a AgentDeck)
  if [[ -n "${OPENAI_APPS_CHALLENGE_TOKEN:-}" ]]; then
    OPEN_ARGS=(--env "OPENAI_APPS_CHALLENGE_TOKEN=${OPENAI_APPS_CHALLENGE_TOKEN}" "${OPEN_ARGS[@]}")
  fi
  open "${OPEN_ARGS[@]}"
  for _ in {1..40}; do
    if curl -sS --max-time 1 "$AGENTDECK_MCP_URL" >/dev/null 2>&1; then
      break
    fi
    sleep 0.5
  done
fi

# Operator UI / Health port
# The tunnel client exposes an admin UI on this port.
# Default: 8081 (changed from 8080 to avoid conflicts on some machines)
# Access it at: http://127.0.0.1:8081/ui

echo "Forwarding tunnel ${OPENAI_TUNNEL_ID} -> ${AGENTDECK_MCP_URL}"
echo "Operator UI: http://127.0.0.1:8081/ui"

exec "$TUNNEL_CLIENT_BIN" run \
  --control-plane.tunnel-id "$OPENAI_TUNNEL_ID" \
  --control-plane.api-key env:OPENAI_API_KEY \
  --mcp.server-url "url=${AGENTDECK_MCP_URL},channel=main" \
  --health.listen-addr 127.0.0.1:8081 \
  --open-web-ui
