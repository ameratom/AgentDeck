#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if lsof -ti:1420 >/dev/null 2>&1; then
  osascript -e 'tell application "AgentDeck" to activate' 2>/dev/null || true
  echo "AgentDeck dev server already running on port 1420 — focused existing window."
  exit 0
fi

exec pnpm tauri dev