# AgentDeck v0.1.0

First notarized macOS release.

## Highlights

- Multi-provider control plane with environment scan and orbital graph
- Streaming chat across LM Studio, Grok/xAI, Anthropic, Codex, and Claude Code
- MCP hub with 11 tools (read + write) over HTTP and stdio
- Handoffs with approval gate and audit logging
- Plugin/skill registry with `execute_skill` pipeline
- Menu bar tray and first-run onboarding
- Grok MCP and AgentDeck MCP connector templates

## Install

1. Download `AgentDeck_0.1.0_aarch64.dmg`
2. Open the DMG and drag **AgentDeck** to Applications
3. Launch AgentDeck — MCP listens on `http://127.0.0.1:7823/mcp`

## Requirements

- macOS 12+
- LM Studio (optional, for local models)
- xAI API key (optional, for Grok)