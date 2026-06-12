#!/usr/bin/env bash
set -euo pipefail

REPO="$(cd "${AGENTDECK_PROJECT_ROOT:-${1:-$(pwd)}}" && pwd)"

if [[ ! -d "$REPO/.git" ]]; then
  echo "Git MCP: $REPO is not a git repository." >&2
  echo "Set AGENTDECK_PROJECT_ROOT or pass the repo path as the first argument." >&2
  exit 1
fi

# MVP policy: read-only usage — prefer status/log/diff tools; avoid commit/push.
exec uvx mcp-server-git --repository "$REPO"