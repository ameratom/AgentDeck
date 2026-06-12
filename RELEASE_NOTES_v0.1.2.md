# AgentDeck v0.1.2

Signed and notarized macOS release with external MCP connectors and Grok bridge sync.

## Highlights

- **Grok MCP bridge** — xAI credentials mirror to a mode-0600 env file for shell launchers
- **Filesystem MCP** — project-scoped read launcher (`filesystem-mcp-launcher.sh`)
- **Git MCP** — read-only repo context launcher (`git-mcp-launcher.sh`)
- Connector templates for Claude Code and Codex under `data/connectors/`
- MCP view: manual **Sync Grok MCP bridge** control

## Install

1. Download `AgentDeck_0.1.2_aarch64.dmg`
2. Open the DMG and drag **AgentDeck** to Applications
3. Launch AgentDeck — MCP listens on `http://127.0.0.1:7823/mcp`

## MCP setup (Claude Code)

Project `.mcp.json` includes `agentdeck`, `grok-mcp`, `filesystem`, and `git`.
Approve each server when Claude Code prompts on first connect.

## Requirements

- macOS 12+
- [uv](https://docs.astral.sh/uv/) (Grok MCP + Git MCP launchers)
- Node/npx (Filesystem MCP launcher)
- LM Studio (optional, for local models)
- xAI API key (optional, for Grok — save in Providers to sync bridge)