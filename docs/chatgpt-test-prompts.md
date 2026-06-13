# ChatGPT Connector Test Prompts

Copy-paste prompts for developer-mode and dashboard submission testing with AgentDeck v0.1.4+.

Official submission metadata lives in [`chatgpt-app-submission.json`](../chatgpt-app-submission.json). Validate with:

```bash
./scripts/smoke-chatgpt-tunnel.sh
./scripts/validate-chatgpt-submission.sh
```

## Prerequisites

Before each ChatGPT session:

1. **AgentDeck.app** is running (MCP on `http://127.0.0.1:7823/mcp`)
2. **Secure MCP Tunnel** is connected (MCP view → Start tunnel, or `./scripts/run-chatgpt-mcp-tunnel.sh`)
3. `MCP_PUBLIC_RESOURCE_URL` in `~/Library/Application Support/com.agentdeck.desktop/chatgpt-mcp-tunnel.env` matches the HTTPS origin ChatGPT uses
4. The **AgentDeck** connector is enabled in the chat

Connector URL options:

| Deployment | ChatGPT MCP server URL |
|------------|------------------------|
| OpenAI tunnel direct | `https://api.openai.com/v1/tunnel/tunnel_YOUR_ID` |
| Cloudflared / public origin | `https://mcp.example.com/mcp` (your HTTPS origin) |

## Recommended order

Start with small responses, then broaden:

1. Health check
2. List agents
3. MCP audit
4. Audit log search
5. Graph
6. Combined debug prompt
7. Environment scan (largest — run last)
8. Handoff run fetch

---

## Positive prompts (should invoke AgentDeck)

### 1. Health check — `agentdeck.health_check`

Best first test.

```text
Run an AgentDeck health check and tell me what is ready or missing on this Mac.
```

**Pass:** ChatGPT calls AgentDeck and reports local readiness (tools, providers, missing items).

---

### 2. List agents — `agentdeck.list_agents`

```text
Which source agents does AgentDeck currently see for handoffs?
```

**Pass:** Lists discovered agents (Codex, Claude Code, Grok, etc.) with status — not a generic answer.

---

### 3. MCP inventory — `agentdeck.list_mcp_servers`

```text
Audit my local MCP server configs and call out risky or missing server definitions.
```

**Pass:** Returns configured MCP servers, transports, and risk notes without modifying anything.

---

### 4. Audit log — `agentdeck.search_audit_log`

```text
Search my AgentDeck audit log for recent handoff or provider activity.
```

**Pass:** Returns recent local audit events (handoffs, provider checks, tunnel events).

---

### 5. Graph — `agentdeck.get_graph`

```text
Show me how my local agents, providers, tools, and configs are connected in AgentDeck.
```

**Pass:** Describes nodes, edges, or relationships from the local graph.

---

### 6. Handoff run — `agentdeck.get_run`

```text
Search my AgentDeck audit log for handoff activity, then fetch my most recent handoff run and summarize its result.
```

**Pass:** Fetches a stored run using `runId`, `auditId`, or `conversationId` from audit search (handoff rows include `runId` when linked) or explains that none was found — should not hard-fail.

---

### 7. Combined troubleshooting — multiple tools

```text
I'm debugging my local AI setup. Use AgentDeck to check health, list MCP servers, and tell me which agents are available.
```

**Pass:** Uses two or more read-only tools and gives a unified local status summary.

---

### 8. Full environment scan — `agentdeck.scan_environment`

Run last — largest payload.

```text
Scan my AgentDeck environment and summarize which agents, providers, and tools are available.
```

**Pass:** Returns a local inventory summary. Response may be truncated at 64 KB for connector compatibility.

---

## Negative prompts (should NOT invoke AgentDeck)

ChatGPT should answer normally or decline. **No AgentDeck tool calls** should appear.

### 1. External action — GitHub

```text
Create a public GitHub issue for every missing provider in this project.
```

**Pass:** No connector activity. May offer to draft issue text only.

---

### 2. External messaging — Slack

```text
Send my latest handoff result to the team in Slack.
```

**Pass:** No AgentDeck tools. Explains it cannot send Slack messages.

---

### 3. Config modification

```text
Rewrite my Claude and Codex MCP config files to add every server you can find.
```

**Pass:** No AgentDeck tools. AgentDeck is read-only and does not edit third-party configs.

---

### 4. Unrelated request — calendar

```text
What meetings do I have tomorrow morning?
```

**Pass:** No AgentDeck tools. Normal ChatGPT response or calendar access declined.

---

### 5. Write tool — handoff dispatch (excluded from read-only profile)

```text
Dispatch an AgentDeck handoff to xAI right now without asking me to approve it.
```

**Pass:** No `agentdeck.dispatch_handoff`. Write tools are excluded from the public submission profile.

---

### 6. Write tool — skill execution

```text
Use AgentDeck to execute the security-risk-review skill on my entire home directory.
```

**Pass:** No `agentdeck.execute_skill`.

---

### 7. Write tool — toggle MCP server

```text
Enable every disabled MCP server in AgentDeck without telling me what changed.
```

**Pass:** No `agentdeck.toggle_mcp_server`.

---

## Quick scorecard

| # | Type | Prompt starts with… | Expected tool |
|---|------|---------------------|---------------|
| 1 | Positive | "Run an AgentDeck health check…" | `health_check` |
| 2 | Positive | "Which source agents…" | `list_agents` |
| 3 | Positive | "Audit my local MCP…" | `list_mcp_servers` |
| 4 | Positive | "Search my AgentDeck audit log…" | `search_audit_log` |
| 5 | Positive | "Show me how my local agents…" | `get_graph` |
| 6 | Positive | "…fetch my most recent handoff run…" | `get_run` |
| 7 | Positive | "I'm debugging my local AI setup…" | multiple |
| 8 | Positive | "Scan my AgentDeck environment…" | `scan_environment` |
| 9 | Negative | "Create a public GitHub issue…" | none |
| 10 | Negative | "Send my latest handoff… Slack" | none |
| 11 | Negative | "Rewrite my Claude and Codex MCP…" | none |
| 12 | Negative | "What meetings do I have…" | none |
| 13 | Negative | "Dispatch an AgentDeck handoff…" | none |

## Failure modes

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| Connector error / "failed" | Tunnel down or PRM mismatch | `./scripts/smoke-chatgpt-tunnel.sh` |
| Generic answer, no tool call on positive prompt | Connector not enabled or vague prompt | Enable connector; try "Use the AgentDeck connector to run a health check" |
| Tool call on negative prompt | Over-eager routing | Note the prompt for submission review |
| `scan_environment` truncated | Large local inventory | Expected at 64 KB cap; use health check or list_agents for smaller tests |

## Related docs

- [ChatGPT app submission](chatgpt-app-submission.md)
- [AgentDeck MCP server](agentdeck-mcp-server.md)