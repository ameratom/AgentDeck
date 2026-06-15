#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DEV_BIN="$ROOT/src-tauri/target/debug/agentdeck"
INSTALLED_BIN="/Applications/AgentDeck.app/Contents/MacOS/agentdeck"

stop_other_agentdeck_instances() {
  osascript -e 'tell application "AgentDeck" to quit' 2>/dev/null || true
  pkill -f "$INSTALLED_BIN" 2>/dev/null || true
  pkill -f "$DEV_BIN" 2>/dev/null || true
  for _ in 1 2 3 4 5; do
    if ! pgrep -f "$INSTALLED_BIN" >/dev/null 2>&1 && ! pgrep -f "$DEV_BIN" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.4
  done
}

focus_dev_window() {
  osascript -e 'tell application "AgentDeck" to activate' 2>/dev/null || true
}

# Reuse an existing dev session when Vite and the debug binary are both up.
if lsof -ti:1420 >/dev/null 2>&1 && pgrep -f "$DEV_BIN" >/dev/null 2>&1; then
  if pgrep -f "$INSTALLED_BIN" >/dev/null 2>&1; then
    pkill -f "$INSTALLED_BIN" 2>/dev/null || true
    sleep 0.5
  fi
  focus_dev_window
  echo "AgentDeck dev session already running — focused existing window."
  exit 0
fi

stop_other_agentdeck_instances
exec pnpm tauri dev