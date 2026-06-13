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

### Claude Code (AgentDeck launcher)

AgentDeck ships a launcher that starts Grok MCP without reading macOS Keychain:

```bash
claude mcp add grok-mcp -- \
  /Users/claudemccready/Desktop/Scripts/Codex/AgentDeck/scripts/grok-mcp-launcher.sh
```

The AgentDeck project `.mcp.json` also includes `grok-mcp` alongside
`agentdeck`.

#### AgentDeck bridge (recommended)

Shell launchers cannot decrypt AgentDeck's encrypted store. When you save an
xAI key in **Providers**, AgentDeck mirrors it to a user-initiated bridge file:

```text
~/Library/Application Support/com.agentdeck.desktop/grok-mcp.env
```

The file is mode `0600` and is read by `scripts/grok-mcp-launcher.sh` after
checking `~/Grok-MCP/.env`. Sync happens automatically on xAI save/import/delete
and on app launch. Use **MCP → Sync Grok MCP bridge** to refresh manually.

Manual key override (optional):

```bash
claude mcp add grok-mcp \
  -e XAI_API_KEY=your_key_here \
  -- uv run --directory ~/Grok-MCP python main.py
```

Templates:

- Claude: `data/connectors/grok-mcp.claude.json`
- Codex: `data/connectors/grok-mcp.codex.toml`

### Codex

```bash
codex mcp add grok-mcp -- \
  /path/to/AgentDeck/scripts/grok-mcp-launcher.sh
```

Or with an explicit key:

```bash
codex mcp add grok-mcp \
  --env XAI_API_KEY=your_key_here \
  -- uv run --directory ~/Grok-MCP python main.py
```

### Useful Grok tools

- `chat`, `grok_agent` — reasoning with tool use
- `web_search`, `x_search` — live research
- `list_models` — model inventory

### AgentDeck xAI Research wrapper

For a smaller read-only research surface, register:

```bash
codex mcp add agentdeck-xai-research-mcp -- \
  /path/to/AgentDeck/scripts/xai-research-mcp-launcher.sh
```

The wrapper reuses AgentDeck's mode-`0600` `grok-mcp.env` bridge and exposes:

- `xai_research.search_web`
- `xai_research.answer_with_sources`
- `xai_research.summarize_url`

Requests use xAI's Responses API with the `web_search` tool and `store: false`.
Only action metadata, duration, model, and source count are written to the local
`xai-research-mcp.audit.jsonl`; prompts and responses are not logged.

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

AgentDeck can generate project-bound exports that include `claude-code` alongside
`agentdeck`, `filesystem`, and `git` from **MCP → Export** or first-run onboarding.
Exports are written under Application Support and are not copied into `.mcp.json`
automatically.

---

## 4. Codex / OpenAI HTTP MCP

Codex supports stdio and streamable HTTP MCP servers. AgentDeck's HTTP transport
satisfies ChatGPT-style remote MCP requirements.

Official docs: [Codex MCP](https://developers.openai.com/codex/mcp)

For OpenAI remote MCP (ChatGPT connectors):

- [OpenAI MCP documentation](https://developers.openai.com/api/docs/guides/tools-connectors-mcp)

Point the remote client at `http://127.0.0.1:7823/mcp` while AgentDeck is running.

---

## 5. Filesystem MCP (project-scoped)

Purpose: read project files safely for handoffs and repo inspection.

MVP policy:

- Scope to explicit project roots only.
- Deny `.env`, `secret.key`, and credential paths in launcher checks.
- Prefer read tools; avoid write tools until Phase 6 approval flow exists.

### Claude Code

```bash
claude mcp add filesystem -- \
  /path/to/AgentDeck/scripts/filesystem-mcp-launcher.sh \
  /path/to/your/project
```

Set `AGENTDECK_PROJECT_ROOT` or `AGENTDECK_FS_ROOTS` (colon-separated) in the
server env for multi-root setups.

Templates:

- `data/connectors/filesystem-mcp.claude.json`
- `data/connectors/filesystem-mcp.codex.toml`

### Verify

```bash
claude mcp list
# filesystem: ... - ✔ Connected
```

---

## 6. Git MCP (read-only MVP)

Purpose: branch/status/log/diff context for handoffs like "review this change."

Uses the official [`mcp-server-git`](https://github.com/modelcontextprotocol/servers/tree/main/src/git)
package via `uvx`. MVP policy: use read-oriented tools only (`git_status`,
`git_log`, `git_diff`, `git_branch`); avoid commit/push until explicit approval.

### Claude Code

```bash
claude mcp add git -- \
  /path/to/AgentDeck/scripts/git-mcp-launcher.sh \
  /path/to/your/project
```

Templates:

- `data/connectors/git-mcp.claude.json`
- `data/connectors/git-mcp.codex.toml`

### Verify

```bash
claude mcp list
# git: ... - ✔ Connected
```

---

## Optional connectors

| Server | Install | Purpose |
|--------|---------|---------|
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
| Grok MCP auth error | Save xAI key in Providers, run **Sync Grok MCP bridge**, or set `XAI_API_KEY` / `~/Grok-MCP/.env` |
| Filesystem MCP path denied | Pass an existing project root; avoid `.env` and secret paths |
| Git MCP not a repository | Point `AGENTDECK_PROJECT_ROOT` at a directory containing `.git` |
| Skill fails with LM Studio 400 | Load a chat model in LM Studio; pick it in Chat preferences |
| Legacy Keychain prompt | Run **Import existing Keychain keys** once and approve the macOS prompt, or enter the API key manually. Normal AgentDeck operation does not access Keychain. |
