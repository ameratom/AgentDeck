# AgentDeck — Codex Handoff

Build folder:

```text
/Users/claudemccready/Desktop/Scripts/Codex/AgentDeck
```

Remote: `https://github.com/ameratom/AgentDeck` (`main`)

Latest release: [v0.1.3](https://github.com/ameratom/AgentDeck/releases/tag/v0.1.3)

## Current Status (June 2026)

Phases **0–11** are complete. v0.1.3 release build is ready to sign, notarize, and publish.

### Shipped

| Area | Status |
|------|--------|
| Environment discovery + orbital graph | ✔ |
| Multi-provider chat (LM Studio, xAI, Anthropic, Codex, Claude Code) | ✔ |
| Encrypted credential store + legacy Keychain import | ✔ |
| MCP inventory + HTTP server (`:7823`) | ✔ |
| External MCP connectors (Grok bridge, filesystem, git) | ✔ |
| xAI Research MCP connector (read-only web research) | ✔ |
| Plugins/skills registry + audit log | ✔ |
| Manual handoffs with approval gate | ✔ |
| Handoff router rules (Settings + suggestions) | ✔ |
| Local project registry + active workspace selection | ✔ |
| Project-scoped config discovery, graph context, chat, and handoffs | ✔ |
| Per-project filesystem/Git MCP profiles with validated exports | ✔ |
| Secure MCP Tunnel controls (MCP view) | ✔ |
| Signed + notarized macOS DMG | ✔ v0.1.2 (v0.1.3 pending publish) |

### Project MCP config (`.mcp.json`)

Registered servers: `agentdeck`, `grok-mcp`, `filesystem`, `git`, `agentdeck-xai-research-mcp` — connect in Claude Code as needed.

Grok auth flows through `~/Library/Application Support/com.agentdeck.desktop/grok-mcp.env` (synced from encrypted xAI credentials).

### Dev commands

```bash
pnpm install
pnpm tauri dev
pnpm test
pnpm test:xai-research-mcp
cd src-tauri && cargo test
```

### Release

```bash
source scripts/notarize.local.env
pnpm tauri build
./scripts/notarize-macos.sh
./scripts/create-github-release.sh v0.1.3
```

Checklist: `RELEASE_CHECKLIST_v0.1.3.md`

## Mission

Build **AgentDeck**, a macOS-first local control plane for AI agents, local LLMs, MCP servers, IDE integrations, skills, plugins, webhooks, and project-specific automations.

AgentDeck should make one thing obvious:

> What is running, what is connected, what is allowed, what changed, and which agent handled which part of the work.

The first release is **observability + controlled chat routing**, not full autonomous orchestration.

## ChatGPT submission (ready to test)

- Import file: `chatgpt-app-submission.json` (read-only v1 profile, 7 tools)
- Guide: `docs/chatgpt-app-submission.md`
- Validate: `./scripts/validate-chatgpt-submission.sh`
- Tunnel helper: `scripts/run-chatgpt-mcp-tunnel.sh` (uses `~/Library/Application Support/com.agentdeck.desktop/chatgpt-mcp-tunnel.env`)
- Tunnel UI: MCP view → Start tunnel → Open operator UI

## Next candidates

1. Publish v0.1.3 signed + notarized DMG to GitHub Releases
2. Submit ChatGPT app via Platform dashboard after tunnel smoke test
3. Phase 12: project-aware onboarding, ChatGPT submission v2 metadata, Claude Code MCP serve export