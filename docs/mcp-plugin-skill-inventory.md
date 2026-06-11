# MCP, Plugins, Skills, and Integration Inventory

## MVP MCPs

### 1. Filesystem MCP

Purpose:

- Read project files.
- Summarize repos.
- Let agents inspect source safely.

MVP policy:

- Read-only.
- Project-scoped.
- Deny `.env`, secrets, private keys, and credential files.

### 2. Git adapter or Git MCP

Purpose:

- Read branch, status, remotes, recent commits, diffs.
- Support handoffs like "review this changed file."

MVP policy:

- Read-only.
- No commit/push until explicit Phase 6 approval flow exists.

### 3. AgentDeck MCP server

Purpose:

- Let Codex, Claude Code, Hermes, and other MCP clients query AgentDeck.

Initial tools:

- `agentdeck.scan_environment`
- `agentdeck.get_graph`
- `agentdeck.list_agents`
- `agentdeck.list_mcp_servers`
- `agentdeck.health_check`
- `agentdeck.get_run`
- `agentdeck.search_audit_log`

Action tools later:

- `agentdeck.create_handoff`
- `agentdeck.start_agent`
- `agentdeck.stop_agent`
- `agentdeck.run_health_check`

### 4. MCP config inspector

Purpose:

- Parse MCP configs.
- Show server command/transport/tools metadata.
- Identify broken paths and risky commands.

This can be internal first, then exposed through MCP later.

## Strongly recommended MCPs

### GitHub MCP

Purpose:

- Issues
- PRs
- repo metadata
- code review context.

### Browser / Playwright MCP

Purpose:

- browser automation
- UI testing
- screenshots
- docs verification.

Enable later because browser automation has high risk.

### xAI Research MCP wrapper

Purpose:

- Let Codex/Claude/Hermes ask Grok for current web research through a controlled adapter.

Name:

```text
agentdeck-xai-research-mcp
```

Tools:

- `xai_research.search_web`
- `xai_research.answer_with_sources`
- `xai_research.summarize_url`

### Webhook MCP/plugin

Purpose:

- Receive events from local or remote systems.
- Trigger graph/run updates.

Examples:

- Git hooks
- CI events
- Slack/Discord later
- OpenClaw channel events.

### Memory MCP or internal memory

Purpose:

- Persistent project notes.
- Handoff summaries.
- Agent preferences.

Policy:

- Project memory only for MVP.
- No personal memory vault until settings and deletion/export controls exist.

## AgentDeck internal plugins

Plugins should start as Rust modules, not a marketplace.

### `agentdeck-plugin-lmstudio`

Detects:

- LM Studio local API
- `/v1/models`
- loaded model IDs
- provider health.

Actions:

- chat
- embeddings later.

### `agentdeck-plugin-codex`

Detects:

- `codex` command
- Codex app availability
- Codex config
- project `AGENTS.md`
- `.codex/`

Actions later:

- create Codex task
- inspect Codex task status
- open Codex project.

### `agentdeck-plugin-claude-code`

Detects:

- `claude` command
- `.claude/settings.json`
- `~/.claude.json`
- `.mcp.json`
- hooks/subagents where configured.

Actions later:

- create Claude Code review task
- query Claude SDK if present.

### `agentdeck-plugin-xai`

Detects:

- xAI API key availability by keychain/env check only.
- model/provider health.

Actions:

- chat
- web research
- function-calling adapter.

### `agentdeck-plugin-hermes`

Detects:

- Hermes command/app
- running process
- config
- MCP config
- skills if documented.

Actions later:

- send task
- monitor long-running work.

### `agentdeck-plugin-openclaw`

Detects:

- `openclaw` command
- gateway status
- MCP registry
- messaging plugins.

Actions later:

- send channel conversation
- expose OpenClaw bridge status.

### `agentdeck-plugin-mcp-inspector`

Detects and validates:

- stdio MCP server commands
- HTTP/SSE servers
- env placeholders
- dangerous args
- duplicate names
- missing executables.

### `agentdeck-plugin-webhooks`

Provides:

- local HTTP endpoint later
- signed webhook validation
- event-to-run mapping.

## Skills

Skills are data files at first.

Suggested path:

```text
skills/
├── agent-inventory.md
├── broken-wire-finder.md
├── graph-explainer.md
├── handoff-planning.md
├── local-llm-chat.md
├── mcp-config-review.md
├── preflight-diagnostics.md
├── repo-summary.md
└── security-risk-review.md
```

### `agent-inventory`

Goal:

- Turn scan output into a clean status summary.

### `mcp-config-review`

Goal:

- Explain MCP configs and identify risk.

### `handoff-planning`

Goal:

- Convert a vague user request into a safe target-agent handoff.

### `local-llm-chat`

Goal:

- Prefer LM Studio/local model for private analysis.

### `repo-summary`

Goal:

- Summarize a project/repo before sending any cloud handoff.

### `security-risk-review`

Goal:

- Flag dangerous edges/capabilities.

### `broken-wire-finder`

Goal:

- Identify disconnected configs, missing commands, broken MCP servers.

## External tools to detect

Commands:

```text
codex
claude
lms
lmstudio
hermes
openclaw
node
npx
uvx
python
python3
pnpm
npm
cargo
rustc
git
gh
```

Processes:

```text
Codex
codex
Claude
claude
Hermes
hermes
OpenClaw
openclaw
LM Studio
lmstudio
lms
node
uvx
python
```
