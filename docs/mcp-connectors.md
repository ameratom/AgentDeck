# MCP Connectors

AgentDeck is the local control plane. External MCP servers extend what each agent can
reach. Wire connectors in this order for the best ROI:

1. **AgentDeck MCP** (already running with the desktop app)
2. **Grok MCP** (free-tier xAI API, agentic tools)
3. **Claude Code MCP** (project-aware coding agent)
4. **Codex MCP** (ChatGPT Plus / OpenAI API tasks)

AgentDeck does not modify third-party config files automatically. Copy the templates
in `data/connectors/` into each tool's config when you are ready.

## Architecture

```
Claude Code / Codex / Grok
        │
        ├─► agentdeck.* tools (HTTP localhost:7823)
        │
        └─► grok-mcp / claude-code-mcp / filesystem (stdio)
```

Write tools (`dispatch_handoff`, `execute_skill`, `toggle_mcp_server`) require
`callerAgentId` and respect the per-agent permission matrix in the Agents view.

| Caller | Default write access |
|--------|---------------------|
| `agent:agentdeck` | All actions |
| `agent:claude-code` | `dispatch-handoff`, `call-mcp-tool` |
| `agent:codex` | `dispatch-handoff`, `call-mcp-tool` |
| `agent:grok` | `dispatch-handoff`, `call-mcp-tool` |

Grant `execute-skill` to a caller in **Agents → Permissions** before that agent can
run `agentdeck.execute_skill`.

---

## 1. AgentDeck MCP (required)

The desktop app starts HTTP MCP on `http://127.0.0.1:7823/mcp`. Stdio is available
for CLI clients.

### Claude Code (project scope)

Add to the project `.mcp.json`:

```json
{
  "mcpServers": {
    "agentdeck": {
      "type": "http",
      "url": "http://127.0.0.1:7823/mcp"
    }
  }
}
```

Template: `data/connectors/agentdeck.mcp.json`

Verify:

```bash
claude mcp list
# agentdeck: http://127.0.0.1:7823/mcp (HTTP) - ✔ Connected
```

### Codex (user scope)

```bash
codex mcp add agentdeck --url http://127.0.0.1:7823/mcp
```

Or add to `~/.codex/config.toml`:

```toml
[mcp_servers.agentdeck]
url = "http://127.0.0.1:7823/mcp"
```

Template: `data/connectors/codex-agentdeck.toml`

### Smoke test

```bash
curl -s -X POST http://127.0.0.1:7823/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"agentdeck.list_agents","arguments":{}}}'
```

---

## 2. Grok MCP (recommended first external connector)

Source: [merterbak/Grok-MCP](https://github.com/merterbak/Grok-MCP)

Prerequisites:

- Python 3.11+
- [uv](https://docs.astral.sh/uv/)
- xAI API key (same key stored in AgentDeck onboarding, or from [console.x.ai](https://console.x.ai))

### Install

```bash
git clone https://github.com/merterbak/Grok-MCP.git ~/Grok-MCP
cd ~/Grok-MCP
uv venv && source .venv/bin/activate
uv sync
```

### Claude Code (recommended: AgentDeck launcher)

AgentDeck ships a launcher that reads the xAI key from the same Keychain entry used
during onboarding (`com.agentdeck.desktop.provider` / `xai`):

```bash
claude mcp add grok-mcp -- \
  /Users/claudemccready/Desktop/Scripts/Codex/AgentDeck/scripts/grok-mcp-launcher.sh
```

The AgentDeck project `.mcp.json` also includes `grok-mcp` alongside `agentdeck`.

Manual key override (optional):

```bash
claude mcp add grok-mcp \
  -e XAI_API_KEY=your_key_here \
  -- uv run --directory ~/Grok-MCP python main.py
```

Template: `data/connectors/grok-mcp.claude.json`

### Codex

```bash
codex mcp add grok-mcp \
  --env XAI_API_KEY=your_key_here \
  -- uv run --directory ~/Grok-MCP python main.py
```

### Useful Grok tools

- `chat`, `grok_agent` — reasoning with tool use
- `web_search`, `x_search` — live research
- `list_models` — model inventory

---

## 3. Claude Code as MCP server

Expose Claude Code so other agents (or AgentDeck handoffs) can delegate coding tasks.

Community reference: [steipete/claude-code-mcp](https://github.com/steipete/claude-code-mcp)

Built-in option (Claude Code 2.x):

```bash
claude mcp serve
```

Register from Codex or another client using the stdio command your Claude install
documents. Billing uses your Claude Agent SDK credit pool.

Template: `data/connectors/claude-code-mcp.json`

---

## 4. Codex / OpenAI HTTP MCP

Codex supports stdio and streamable HTTP MCP servers. AgentDeck's HTTP transport
satisfies ChatGPT-style remote MCP requirements.

Official docs: [Codex MCP](https://developers.openai.com/codex/mcp)

For OpenAI remote MCP (ChatGPT connectors):

- [OpenAI MCP documentation](https://developers.openai.com/api/docs/guides/tools-connectors-mcp)

Point the remote client at `http://127.0.0.1:7823/mcp` while AgentDeck is running.

---

## Optional connectors

| Server | Install | Purpose |
|--------|---------|---------|
| Filesystem | `npx -y @modelcontextprotocol/server-filesystem ~/Desktop ~/Downloads` | Safe file read for vision/file paths |
| Playwright | `npx -y @playwright/mcp@latest` | Browser automation (high risk — enable later) |
| GitHub | `npx -y @modelcontextprotocol/server-github` | Issues, PRs, repo metadata |

---

## Write-tool validation from Claude Code

After AgentDeck is running and `.mcp.json` includes the HTTP server:

### Handoff (already validated)

Ask Claude Code:

> Use `agentdeck.dispatch_handoff` with `callerAgentId: agent:claude-code` to send a
> one-line task to LM Studio (`qwen/qwen3.5-9b`). Report the run ID and audit ref.

### Skill execution

Grant `execute-skill` to `agent:claude-code` in the Agents permissions matrix, then:

> Call `agentdeck.execute_skill` with `skillId: local-llm-chat` and
> `callerAgentId: agent:claude-code`. Summarize the skill output and audit ref.

Or use `agent:agentdeck` as caller (has `execute-skill` by default):

```bash
curl -s -X POST http://127.0.0.1:7823/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"agentdeck.execute_skill","arguments":{"skillId":"local-llm-chat","callerAgentId":"agent:agentdeck"}}}'
```

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `agentdeck` MCP failed to connect | Launch AgentDeck.app (HTTP server starts with the app) |
| `permission denied: agent:claude-code cannot perform execute-skill` | Enable `execute-skill` in Agents → Permissions |
| Grok MCP auth error | Set `XAI_API_KEY` or add `.env` in the Grok-MCP directory |
| Skill fails with LM Studio 400 | Load a chat model in LM Studio; pick it in Chat preferences |
| Keychain prompt loops | Use your **Mac login password**; AgentDeck stores provider keys under service `com.agentdeck.desktop.provider` |