# AgentDeck v0.1.1

Signed and notarized macOS release with provider auth hardening and MCP fixes.

## Highlights

- Encrypted local credential store (`provider_secrets` + `secret.key`) — no runtime Keychain reads
- One-time legacy Keychain import with per-provider outcomes and `import-failed` status
- MCP HTTP server fix: tool calls no longer crash under Tokio (Claude Code + Codex validated)
- Hermes `config.yaml` parsing in environment discovery
- Handoff dispatch and provider catalog verification improvements

## Install

1. Download `AgentDeck_0.1.1_aarch64.dmg`
2. Open the DMG and drag **AgentDeck** to Applications
3. Launch AgentDeck — MCP listens on `http://127.0.0.1:7823/mcp`

## Requirements

- macOS 12+
- LM Studio (optional, for local models)
- xAI API key (optional, for Grok)