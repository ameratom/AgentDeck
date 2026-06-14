#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_SUPPORT="${HOME}/Library/Application Support/com.agentdeck.desktop"
LOCAL_ENV="${AGENTDECK_TUNNEL_ENV:-$APP_SUPPORT/chatgpt-mcp-tunnel.env}"
RUNTIME_DIR="$APP_SUPPORT/tunnel-client"
MCP_URL="${AGENTDECK_MCP_URL:-http://127.0.0.1:7823/mcp}"
HEALTH_ADDR="127.0.0.1:8081"
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
  local payload="$1"
  curl -sS --max-time 30 -X POST "$MCP_URL" \
    -H 'Content-Type: application/json' \
    -d "$payload"
}

mcp_call() {
  local tool="$1"
  local args="${2:-"{}"}"
  mcp_post "$(jq -nc --arg tool "$tool" --argjson args "$args" \
    '{jsonrpc:"2.0",id:1,method:"tools/call",params:{name:$tool,arguments:$args}}')"
}

echo "AgentDeck ChatGPT tunnel smoke test"
echo

echo "1. Local MCP endpoint"
if mcp_post '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | jq -e '.result.tools | length > 0' >/dev/null; then
  tool_count="$(mcp_post '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
    | jq '.result.tools | length')"
  pass "tools/list returned $tool_count tools at $MCP_URL"
  if [[ "$tool_count" == "10" ]]; then
    pass "tools/list matches ChatGPT read_only_v1_1 profile (10 tools)"
  else
    fail "expected 10 submission tools at $MCP_URL (got $tool_count) — restart AgentDeck after upgrading"
  fi
  if mcp_post '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
    | jq -e '.result.tools[].name | select(. == "agentdeck.xai_research_search_web")' >/dev/null; then
    pass "tools/list includes xAI research tools"
  else
    fail "tools/list missing agentdeck.xai_research_search_web"
  fi
else
  fail "tools/list failed at $MCP_URL — launch AgentDeck first"
  echo
  echo "Result: $FAIL failed, $PASS passed"
  exit 1
fi

submitted_tools=(
  agentdeck.scan_environment
  agentdeck.get_graph
  agentdeck.list_agents
  agentdeck.list_mcp_servers
  agentdeck.health_check
  agentdeck.search_audit_log
  agentdeck.xai_research_search_web
  agentdeck.xai_research_answer_with_sources
  agentdeck.xai_research_summarize_url
)

echo
echo "2. Read-only submission tools (local MCP)"
for tool in "${submitted_tools[@]}"; do
  case "$tool" in
    agentdeck.search_audit_log)
      response="$(mcp_call "$tool" '{"query":"handoff","limit":5}')"
      ;;
    agentdeck.xai_research_search_web)
      response="$(mcp_call "$tool" '{"query":"ameratom/AgentDeck macOS control plane MCP release notes","maxSources":3}')"
      ;;
    agentdeck.xai_research_answer_with_sources)
      response="$(mcp_call "$tool" '{"question":"What MCP transports do ChatGPT connectors support?","maxSources":3}')"
      ;;
    agentdeck.xai_research_summarize_url)
      response="$(mcp_call "$tool" '{"url":"https://developers.openai.com/api/docs/guides/tools-connectors-mcp"}')"
      ;;
    *)
      response="$(mcp_call "$tool" '{}')"
      ;;
  esac
  if echo "$response" | jq -e '.result.content[0].text | length > 0' >/dev/null 2>&1; then
    pass "$tool returned content"
  elif echo "$response" | jq -e '.result | keys | length > 0' >/dev/null 2>&1; then
    pass "$tool returned structured result"
  elif [[ "$tool" == agentdeck.xai_research_* ]] \
    && echo "$response" | jq -r '.error.message // .result.content[0].text // empty' \
      | rg -qi 'xai|api key|not configured'; then
    pass "$tool skipped (xAI key not configured locally)"
  else
    detail="$(echo "$response" | jq -r '.error.message // .result.isError // "unknown error"' 2>/dev/null || echo "invalid JSON")"
    fail "$tool — $detail"
  fi
done

run_id=""
if command -v sqlite3 >/dev/null 2>&1; then
  db_path="$APP_SUPPORT/agentdeck.sqlite3"
  if [[ -f "$db_path" ]]; then
    run_id="$(sqlite3 "$db_path" "SELECT id FROM handoff_runs ORDER BY created_at DESC LIMIT 1;" 2>/dev/null | tr -d '[:space:]' || true)"
  fi
fi
if [[ -n "$run_id" ]]; then
  get_run_response="$(mcp_call agentdeck.get_run "$(jq -nc --arg runId "$run_id" '{runId:$runId}')")"
  if echo "$get_run_response" | jq -e '.result.content[0].text | length > 0' >/dev/null 2>&1; then
    pass "agentdeck.get_run returned stored run $run_id"
  else
    fail "agentdeck.get_run failed for stored run $run_id"
  fi
else
  pass "agentdeck.get_run skipped (no stored handoff runs yet)"
fi

echo
echo "3. Tunnel configuration"
if [[ ! -f "$LOCAL_ENV" ]]; then
  fail "Missing tunnel env: $LOCAL_ENV"
  echo
  echo "Copy scripts/chatgpt-mcp-tunnel.example.env and fill in OPENAI_TUNNEL_ID + OPENAI_API_KEY"
  echo "Result: $FAIL failed, $PASS passed"
  exit 1
fi

# shellcheck disable=SC1091
set -a
source "$LOCAL_ENV"
set +a

if [[ "${OPENAI_TUNNEL_ID:-}" == "tunnel_..." || "${OPENAI_TUNNEL_ID:-}" == *"YOUR_TUNNEL_ID_HERE"* ]]; then
  fail "OPENAI_TUNNEL_ID is still a placeholder in $LOCAL_ENV"
elif [[ -z "${OPENAI_TUNNEL_ID:-}" ]]; then
  fail "OPENAI_TUNNEL_ID is not set in $LOCAL_ENV"
else
  pass "OPENAI_TUNNEL_ID is configured"
fi

if [[ "${OPENAI_API_KEY:-}" == "sk-..." || "${OPENAI_API_KEY:-}" == *"YOUR_OPENAI_API_KEY_HERE"* ]]; then
  fail "OPENAI_API_KEY is still a placeholder in $LOCAL_ENV"
elif [[ -z "${OPENAI_API_KEY:-}" ]]; then
  fail "OPENAI_API_KEY is not set in $LOCAL_ENV"
else
  pass "OPENAI_API_KEY is configured"
fi

TUNNEL_CLIENT_BIN="${TUNNEL_CLIENT_BIN:-tunnel-client}"
if command -v "$TUNNEL_CLIENT_BIN" >/dev/null 2>&1; then
  pass "tunnel-client found ($TUNNEL_CLIENT_BIN)"
else
  fail "tunnel-client not found — install OpenAI tunnel-client or set TUNNEL_CLIENT_BIN"
fi

echo
echo "4. Secure MCP Tunnel"
mkdir -p "$RUNTIME_DIR"
PID_FILE="$RUNTIME_DIR/tunnel-client.pid"
LOG_FILE="$RUNTIME_DIR/tunnel-client.log"
HEALTH_URL_FILE="$RUNTIME_DIR/health-url.txt"

tunnel_running() {
  if [[ -f "$PID_FILE" ]]; then
    local pid
    pid="$(tr -d '[:space:]' <"$PID_FILE")"
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      return 0
    fi
  fi
  return 1
}

if tunnel_running && curl -sS --max-time 2 "http://$HEALTH_ADDR/readyz" >/dev/null 2>&1; then
  pass "tunnel already running and ready"
else
  if tunnel_running; then
    kill "$(tr -d '[:space:]' <"$PID_FILE")" 2>/dev/null || true
    sleep 1
  fi
  rm -f "$PID_FILE" "$HEALTH_URL_FILE"
  echo "  … starting tunnel-client"
  "$TUNNEL_CLIENT_BIN" run \
    --control-plane.tunnel-id "$OPENAI_TUNNEL_ID" \
    --control-plane.api-key env:OPENAI_API_KEY \
    --mcp.server-url "url=${AGENTDECK_MCP_URL:-http://127.0.0.1:7823/mcp},channel=main" \
    --health.listen-addr "$HEALTH_ADDR" \
    --health.url-file "$HEALTH_URL_FILE" \
    --pid.file "$PID_FILE" \
    --log.file "$LOG_FILE" \
    >/dev/null 2>&1 &
  for _ in {1..30}; do
    if curl -sS --max-time 2 "http://$HEALTH_ADDR/readyz" >/dev/null 2>&1; then
      break
    fi
    sleep 0.5
  done
  if curl -sS --max-time 2 "http://$HEALTH_ADDR/readyz" >/dev/null 2>&1; then
    pass "tunnel-client started and /readyz is healthy"
  else
    fail "tunnel-client did not become ready — check $LOG_FILE"
  fi
fi

if curl -sS --max-time 2 "http://$HEALTH_ADDR/ui" | rg -q "tunnel|Tunnel|MCP" 2>/dev/null; then
  pass "operator UI reachable at http://$HEALTH_ADDR/ui"
else
  fail "operator UI did not respond at http://$HEALTH_ADDR/ui"
fi

PUBLIC_URL=""
if [[ -f "$LOG_FILE" ]]; then
  PUBLIC_URL="$(rg -o 'tunnel_url=https://[^ ]+' "$LOG_FILE" 2>/dev/null | tail -1 | cut -d= -f2- || true)"
fi
if [[ -z "$PUBLIC_URL" ]]; then
  PUBLIC_URL="https://api.openai.com/v1/tunnel/${OPENAI_TUNNEL_ID}"
fi

pass "public MCP URL for dashboard: $PUBLIC_URL"

if [[ -f "$LOG_FILE" ]] && rg -q "mcp session initialized" "$LOG_FILE" 2>/dev/null; then
  server_version="$(rg -o 'server_version=[0-9.]+' "$LOG_FILE" 2>/dev/null | tail -1 | cut -d= -f2 || echo unknown)"
  pass "tunnel probed local MCP (server_version=$server_version)"
else
  fail "tunnel log missing successful MCP probe — check $LOG_FILE"
fi

echo
echo "5. OAuth protected-resource metadata"
prm_resource="$(curl -sS --max-time 10 "http://127.0.0.1:7823/.well-known/oauth-protected-resource/mcp" | jq -r '.resource // empty')"
if [[ -n "$prm_resource" ]]; then
  pass "local PRM resource: $prm_resource"
else
  fail "local PRM metadata missing resource field"
fi

expected_public="${MCP_PUBLIC_RESOURCE_URL:-}"
if [[ -z "$expected_public" && "${AGENTDECK_MCP_URL:-}" == https://* ]]; then
  expected_public="${AGENTDECK_MCP_URL}"
fi
if [[ -n "$expected_public" ]]; then
  expected_public="${expected_public%/}"
  if [[ "$prm_resource" == "$expected_public" ]]; then
    pass "PRM resource matches configured public MCP URL"
  else
    fail "PRM resource mismatch (expected $expected_public, got $prm_resource)"
  fi
fi

if [[ "${AGENTDECK_MCP_URL:-}" == https://* ]]; then
  remote_prm="$(curl -sS --max-time 10 "${AGENTDECK_MCP_URL%/}/../.well-known/oauth-protected-resource/mcp" 2>/dev/null | jq -r '.resource // empty' || true)"
  remote_origin="$(echo "${AGENTDECK_MCP_URL}" | sed -E 's#/mcp$##')"
  remote_prm="$(curl -sS --max-time 10 "${remote_origin}/.well-known/oauth-protected-resource/mcp" | jq -r '.resource // empty' 2>/dev/null || true)"
  if [[ -n "$remote_prm" && "$remote_prm" == "$expected_public" ]]; then
    pass "public origin PRM matches configured public MCP URL"
  elif [[ -n "$remote_prm" ]]; then
    fail "public origin PRM mismatch (expected ${expected_public:-$remote_prm}, got $remote_prm)"
  fi
fi

echo
echo "6. Submission validator"
if "$ROOT_DIR/scripts/validate-chatgpt-submission.sh" >/dev/null 2>&1; then
  pass "chatgpt-app-submission.json validates"
else
  fail "submission validator failed"
fi

echo
echo "Smoke test summary: $PASS passed, $FAIL failed"
echo
echo "ChatGPT dashboard steps:"
echo "  1. Open https://platform.openai.com/apps (or Connectors in ChatGPT dev mode)"
echo "  2. MCP server URL: $PUBLIC_URL"
echo "  3. Import metadata from chatgpt-app-submission.json"
echo "  4. Operator UI: http://$HEALTH_ADDR/ui"
echo "  5. Run positive prompts from chatgpt-app-submission.json test_cases"
echo "  6. Confirm negative_test_cases do NOT invoke AgentDeck"

if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi