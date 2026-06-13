# AgentDeck v0.1.5 — Phase 13

Grok-first integration, ChatGPT v1.1 research tools, and fixes from the ChatGPT prompt test seam.

## Highlights

- **Handoff run lookup** — `get_run` accepts `runId`, `auditId`, or `conversationId`; audit search enriches `handoff.dispatch` rows with `runId`
- **Audit search** — OR-friendly queries (`handoff OR provider`, `handoff provider`)
- **PATH enrichment** — GUI app scans merge login-shell PATH, common macOS paths, and process-derived CLI locations
- **Grok connector exports** — Project profiles can export `grok-mcp` and `agentdeck-xai-research-mcp` (MCP view + onboarding)
- **Grok-first routing** — xAI fallback when no keyword rule matches; fresh installs route code/implement tasks to Grok
- **ChatGPT v1.1** — Read-only xAI Research tools proxied through AgentDeck HTTP MCP:
  - `agentdeck.xai_research_search_web`
  - `agentdeck.xai_research_answer_with_sources`
  - `agentdeck.xai_research_summarize_url`

## ChatGPT re-test

Re-run prompts 4 and 6 from `docs/chatgpt-test-prompts.md` after upgrading. Research tools require an xAI API key saved in AgentDeck Settings.

```bash
./scripts/smoke-chatgpt-tunnel.sh
```

## Install

1. Download `AgentDeck_0.1.5_aarch64.dmg`
2. Open the DMG and drag **AgentDeck** to Applications
3. Launch AgentDeck — MCP listens on `http://127.0.0.1:7823/mcp`