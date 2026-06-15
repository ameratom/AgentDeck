#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DEV_MATCH="Codex/AgentDeck/src-tauri/target/debug/agentdeck"
INSTALLED_MATCH="/Applications/AgentDeck.app/Contents/MacOS/agentdeck"

dev_running() {
  pgrep -f "$DEV_MATCH" >/dev/null 2>&1
}

installed_running() {
  pgrep -f "$INSTALLED_MATCH" >/dev/null 2>&1
}

vite_running() {
  lsof -ti:1420 >/dev/null 2>&1
}

stop_installed_app() {
  pkill -f "$INSTALLED_MATCH" 2>/dev/null || true
}

stop_dev_app() {
  pkill -f "$DEV_MATCH" 2>/dev/null || true
}

stop_vite() {
  lsof -ti:1420 2>/dev/null | xargs kill 2>/dev/null || true
}

stop_all_agentdeck_gui() {
  osascript -e 'tell application "AgentDeck" to quit' 2>/dev/null || true
  stop_installed_app
  stop_dev_app
  for _ in 1 2 3 4 5; do
    if ! installed_running && ! dev_running; then
      return 0
    fi
    sleep 0.4
  done
}

focus_dev_window() {
  osascript -e 'tell application "AgentDeck" to activate' 2>/dev/null || true
}

# Healthy dev session: Vite is up and the debug binary is running.
if vite_running && dev_running; then
  if installed_running; then
    stop_installed_app
    sleep 0.5
  fi
  focus_dev_window
  echo "AgentDeck dev session already running — focused existing window."
  exit 0
fi

# Orphaned Vite without the debug app — clear the port before relaunching.
if vite_running && ! dev_running; then
  stop_vite
  sleep 0.5
fi

stop_all_agentdeck_gui
exec pnpm tauri dev