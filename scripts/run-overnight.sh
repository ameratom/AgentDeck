#!/bin/bash
set -euo pipefail

# AgentDeck Overnight Runner
# Usage: EXECUTE_VERIFY=0 AGENTDECK_COMPOSER_BRIDGE=dry-run ./scripts/run-overnight.sh

cd "$(dirname "$0")/.."

export PATH="/Users/claudemccready/.npm-global/bin:$PATH"

# Load Cursor / composer bridge credentials
if [ -f scripts/composer-bridge.local.env ]; then
  source scripts/composer-bridge.local.env
fi

EXECUTE_VERIFY=${EXECUTE_VERIFY:-1}
AGENTDECK_COMPOSER_BRIDGE=${AGENTDECK_COMPOSER_BRIDGE:-cursor-agent}

echo "=== AgentDeck Overnight Run ==="
echo "EXECUTE_VERIFY=$EXECUTE_VERIFY"
echo "AGENTDECK_COMPOSER_BRIDGE=$AGENTDECK_COMPOSER_BRIDGE"
echo ""

cargo run --manifest-path src-tauri/Cargo.toml --bin hermes -- overnight --queue tasks/overnight.queue.json
