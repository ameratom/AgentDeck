#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DEV_BIN="$ROOT/src-tauri/target/debug/agentdeck"
INSTALLED_BIN="/Applications/AgentDeck.app/Contents/MacOS/agentdeck"

dev_pids() {
  local pid cwd
  while read -r pid; do
    [[ -z "$pid" ]] && continue
    cwd="$(lsof -a -p "$pid" -d cwd -Fn 2>/dev/null | sed -n 's/^n//p' || true)"
    if [[ "$cwd" == "$ROOT"* ]]; then
      echo "$pid"
    fi
  done < <(pgrep -f "target/debug/agentdeck" 2>/dev/null || true)
}

installed_pids() {
  if [[ -f "$INSTALLED_BIN" ]]; then
    lsof -t "$INSTALLED_BIN" 2>/dev/null || true
  fi
}

dev_running() {
  [[ -n "$(dev_pids)" ]]
}

installed_running() {
  [[ -n "$(installed_pids)" ]]
}

vite_running() {
  lsof -ti:1420 >/dev/null 2>&1
}

stop_pids() {
  local pids="$1"
  if [[ -n "$pids" ]]; then
    kill $pids 2>/dev/null || true
  fi
}

stop_installed_app() {
  stop_pids "$(installed_pids)"
}

stop_dev_app() {
  stop_pids "$(dev_pids)"
}

stop_vite() {
  lsof -ti:1420 2>/dev/null | xargs kill 2>/dev/null || true
}

stop_all_agentdeck_gui() {
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
  # Do not use `tell application "AgentDeck"` — macOS launches /Applications/AgentDeck.app.
  local pid
  pid="$(dev_pids | head -n 1)"
  if [[ -n "$pid" ]]; then
    osascript -e "tell application \"System Events\" to set frontmost of first process whose unix id is ${pid} to true" 2>/dev/null || true
  fi
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

# Debug app without Vite — stale GUI after the dev server died.
if dev_running && ! vite_running; then
  echo "Restarting stale AgentDeck dev session..."
fi

# Never keep the installed .app copy during local development.
if installed_running; then
  stop_installed_app
  sleep 0.5
fi

stop_all_agentdeck_gui
exec pnpm tauri dev