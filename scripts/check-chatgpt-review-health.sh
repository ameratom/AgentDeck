#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_MCP_URL="${AGENTDECK_MCP_URL:-http://127.0.0.1:7823/mcp}"
APP_SUPPORT="${HOME}/Library/Application Support/com.agentdeck.desktop"
LOCAL_ENV="${AGENTDECK_TUNNEL_ENV:-$APP_SUPPORT/chatgpt-mcp-tunnel.env}"
PASS=0
FAIL=0

pass() {
  echo "  ✔ $1"
  PASS=$((PASS + 1))
}

fail() {
  echo "  ✘ $1"
  FAIL=$((FAIL + 1))
}

mcp_post() {
  local url="$1"
  local payload="$2"
  curl -sS --max-time 15 -X POST "$url" \
    -H 'Content-Type: application/json' \
    -d "$payload"
}

echo "ChatGPT review health check"
echo

if command -v jq >/dev/null 2>&1; then
  if HEALTH_JSON="$(cargo run --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" --bin hermes -- review 2>/dev/null)"; then
    pass "Hermes review command reports ready for reviewers"
    echo "$HEALTH_JSON" | jq -r '.checks[] | select(.passed == false and (.id != "publish-gate")) | "  ⚠ \(.label): \(.detail)"' || true
  else
    fail "Hermes review command reported action needed"
    echo "$HEALTH_JSON" | jq -r '.checks[] | select(.passed == false) | "  ⚠ \(.label): \(.detail)"' 2>/dev/null || true
  fi
else
  echo "  … jq not found; skipping Hermes JSON summary"
fi

echo
echo "Local MCP contract"
TOOL_JSON="$(mcp_post "$LOCAL_MCP_URL" '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' || true)"
if [[ -n "$TOOL_JSON" ]] && echo "$TOOL_JSON" | jq -e '.result.tools | length == 10' >/dev/null 2>&1; then
  pass "local tools/list returns 10 submission tools"
else
  fail "local tools/list did not return 10 tools at $LOCAL_MCP_URL"
fi

if echo "$TOOL_JSON" | jq -e '.result.tools[].name | select(test("dispatch_handoff|execute_skill|toggle_mcp_server"))' >/dev/null 2>&1; then
  fail "local tools/list still exposes deferred write tools"
else
  pass "deferred write tools are not exposed locally"
fi

PUBLIC_URL=""
if [[ -f "$LOCAL_ENV" ]]; then
  # shellcheck disable=SC1091
  set -a
  source "$LOCAL_ENV"
  set +a
  PUBLIC_URL="${MCP_PUBLIC_RESOURCE_URL:-}"
  if [[ -z "$PUBLIC_URL" && "${AGENTDECK_MCP_URL:-}" == https://* ]]; then
    PUBLIC_URL="${AGENTDECK_MCP_URL}"
  fi
fi

echo
echo "Public MCP endpoint"
if [[ -n "$PUBLIC_URL" ]]; then
  PUBLIC_JSON="$(mcp_post "$PUBLIC_URL" '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' || true)"
  if [[ -n "$PUBLIC_JSON" ]] && echo "$PUBLIC_JSON" | jq -e '.result.tools | length == 10' >/dev/null 2>&1; then
    pass "public tools/list returned 10 tools at $PUBLIC_URL"
  else
    fail "public tools/list failed at $PUBLIC_URL"
  fi
else
  fail "MCP_PUBLIC_RESOURCE_URL not configured in $LOCAL_ENV"
fi

echo
echo "Review health summary: $PASS passed, $FAIL failed"
echo "Platform status: REVIEW — publishing stays blocked until OpenAI approves."

if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi