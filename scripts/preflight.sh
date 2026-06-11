#!/usr/bin/env bash
set -euo pipefail

echo "=== AgentDeck Preflight ==="

check_cmd() {
  local name="$1"
  local version_arg="${2:---version}"

  if command -v "$name" >/dev/null 2>&1; then
    echo "✓ $name: $($name $version_arg 2>&1 | head -n 1)"
  else
    echo "✗ $name: unavailable"
  fi
}

check_cmd node --version
check_cmd pnpm --version
check_cmd rustc --version
check_cmd cargo --version
check_cmd git --version
check_cmd codex --version
check_cmd claude --version
check_cmd lms --version
check_cmd hermes --version
check_cmd openclaw --version

echo ""
echo "=== LM Studio ==="
if curl -s --max-time 2 http://localhost:1234/v1/models >/tmp/agentdeck-lmstudio-models.json 2>/dev/null; then
  echo "✓ LM Studio API reachable at http://localhost:1234/v1"
  cat /tmp/agentdeck-lmstudio-models.json
  echo ""
else
  echo "✗ LM Studio API unavailable at http://localhost:1234/v1"
fi

echo ""
echo "=== Agent-like processes ==="
ps aux | grep -Ei "codex|claude|hermes|openclaw|lm studio|lmstudio|lms|mcp|uvx|npx" | grep -v grep || true
