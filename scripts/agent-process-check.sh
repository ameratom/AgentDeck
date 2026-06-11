#!/usr/bin/env bash
set -euo pipefail

echo "=== AI Agent Process Check ==="
pgrep -afil "codex|claude|hermes|openclaw|lm studio|lmstudio|lms|mcp|uvx|npx" || echo "No matching AI agent processes found."

echo ""
echo "=== launchd Services ==="
launchctl list | grep -Ei "codex|claude|hermes|openclaw|lmstudio|lms|mcp" || echo "No matching launchd services found."
