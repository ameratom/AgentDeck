# AgentDeck — Codex Handoff

Build folder:

```text
/Users/claudemccready/Desktop/Scripts/Codex/AgentDeck
```

Remote: `https://github.com/ameratom/AgentDeck` (`main`)

Latest release: [v0.1.2](https://github.com/ameratom/AgentDeck/releases/tag/v0.1.2)

## Current Status (June 2026)

Phases **0–9** are complete. Phase **10** external MCP connectors shipped in v0.1.2.

### Shipped

| Area | Status |
|------|--------|
| Environment discovery + orbital graph | ✔ |
| Multi-provider chat (LM Studio, xAI, Anthropic, Codex, Claude Code) | ✔ |
| Encrypted credential store + legacy Keychain import | ✔ |
| MCP inventory + HTTP server (`:7823`) | ✔ |
| External MCP connectors (Grok bridge, filesystem, git) | ✔ |
| Plugins/skills registry + audit log | ✔ |
| Manual handoffs with approval gate | ✔ |
| Handoff router rules (Settings + suggestions) | ✔ |
| Signed + notarized macOS DMG | ✔ v0.1.2 |

### Project MCP config (`.mcp.json`)

Registered servers: `agentdeck`, `grok-mcp`, `filesystem`, `git` — all connected in Claude Code.

Grok auth flows through `~/Library/Application Support/com.agentdeck.desktop/grok-mcp.env` (synced from encrypted xAI credentials).

### Dev commands

```bash
pnpm install
pnpm tauri dev
pnpm test
cd src-tauri && cargo test
```

### Release

```bash
source scripts/notarize.local.env
pnpm tauri build
./scripts/notarize-macos.sh
./scripts/create-github-release.sh v0.1.2
```

## Mission

Build **AgentDeck**, a macOS-first local control plane for AI agents, local LLMs, MCP servers, IDE integrations, skills, plugins, webhooks, and project-specific automations.

AgentDeck should make one thing obvious:

> What is running, what is connected, what is allowed, what changed, and which agent handled which part of the work.

The first release is **observability + controlled chat routing**, not full autonomous orchestration.

## Next candidates

1. ChatGPT app submission polish (`chatgpt-app-submission.json`)
2. Chat view router suggestions (parity with Handoffs)
3. xAI Research MCP wrapper (`agentdeck-xai-research-mcp`)
4. Router rules: seed sensible defaults on first run