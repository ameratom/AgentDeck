# AgentDeck v0.1.3

Signed and notarized macOS release with project workspaces, scoped control plane features, Secure MCP Tunnel controls, and xAI Research MCP connector.

## Highlights

- **Project workspaces** — register local folders, pick an active project, audited add/remove
- **Scoped control plane** — discovery graph, chat, handoffs, and MCP exports respect the active project
- **Per-project connector exports** — validated Claude Code and Codex profiles for filesystem, git, Grok, and AgentDeck MCP
- **Secure MCP Tunnel UI** — start/stop tunnel and open operator UI from the MCP view (ChatGPT connector path)
- **xAI Research MCP** — read-only web research wrapper (`xai_research.search_web`, `answer_with_sources`, `summarize_url`) using the encrypted Grok bridge

## Install

1. Download `AgentDeck_0.1.3_aarch64.dmg`
2. Open the DMG and drag **AgentDeck** to Applications
3. Launch AgentDeck — MCP listens on `http://127.0.0.1:7823/mcp`

## Getting started with projects

1. Open **Projects** and register your repo path
2. Set it as the active workspace
3. Re-scan from the graph or switch views — chat and handoffs stay scoped to that root

## MCP setup (Claude Code)

Project `.mcp.json` can include `agentdeck`, `grok-mcp`, `filesystem`, `git`, and optionally `agentdeck-xai-research-mcp`.
Use **MCP → Export** for per-project connector snippets after selecting a workspace.

## ChatGPT connector

1. Configure `~/Library/Application Support/com.agentdeck.desktop/chatgpt-mcp-tunnel.env`
2. In **MCP**, start the Secure MCP Tunnel and open the operator UI for the public HTTPS URL
3. See `docs/chatgpt-app-submission.md` for dashboard submission steps

## Requirements

- macOS 12+
- [uv](https://docs.astral.sh/uv/) (Grok MCP + Git MCP launchers)
- Node/npx (Filesystem MCP + xAI Research MCP launchers)
- LM Studio (optional, for local models)
- xAI API key (optional, for Grok and xAI Research — save in Providers to sync bridge)