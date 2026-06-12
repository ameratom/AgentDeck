# ChatGPT App Submission

AgentDeck exposes a local MCP server for ChatGPT via [Secure MCP Tunnel](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels) or developer-mode connectors.

Submission import file: [`chatgpt-app-submission.json`](../chatgpt-app-submission.json)

## Submission profile (v0.1.2)

This submission includes **read-only** MCP tools only:

| Tool | Purpose |
|------|---------|
| `agentdeck.scan_environment` | Tools, providers, processes, configs |
| `agentdeck.get_graph` | Relationship graph snapshot |
| `agentdeck.list_agents` | Discovered local agents |
| `agentdeck.list_mcp_servers` | MCP inventory + risk labels |
| `agentdeck.health_check` | Local readiness / preflight |
| `agentdeck.get_run` | One stored handoff run |
| `agentdeck.search_audit_log` | Audit search |

Write tools (`dispatch_handoff`, `execute_skill`, `toggle_mcp_server`) stay in **developer mode** until a follow-up submission adds permission and approval UX copy.

## Prerequisites

1. **AgentDeck.app** running (HTTP MCP on `http://127.0.0.1:7823/mcp`)
2. **ChatGPT developer mode** enabled (Settings → Apps & Connectors → Advanced)
3. For public submission review: **Secure MCP Tunnel** or another HTTPS path ChatGPT can reach
4. **Organization verification** in [OpenAI Platform](https://platform.openai.com/settings/organization/general)

## Developer mode (fastest test)

1. Launch AgentDeck.
2. In ChatGPT: Settings → Connectors → Create.
3. Connection type: **Tunnel** (recommended) or HTTPS proxy to localhost.
4. Local endpoint: `http://127.0.0.1:7823/mcp`
5. Add connector metadata from `chatgpt-app-submission.json` → `connector` and `app_info`.
6. Start a chat, enable the AgentDeck connector, and run a test prompt from `test_cases`.

Smoke test from terminal:

```bash
curl -s -X POST http://127.0.0.1:7823/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

## Secure MCP Tunnel setup

AgentDeck listens on loopback only. ChatGPT cannot call `http://127.0.0.1` directly; use OpenAI's tunnel client inside your network.

1. Create a tunnel in [Platform tunnel settings](https://platform.openai.com/settings/organization/tunnels).
2. Copy `scripts/chatgpt-mcp-tunnel.example.env` → `scripts/chatgpt-mcp-tunnel.local.env` and fill in values.
3. With AgentDeck running:

```bash
source scripts/chatgpt-mcp-tunnel.local.env
./scripts/run-chatgpt-mcp-tunnel.sh
```

4. Register the OpenAI-hosted tunnel URL in ChatGPT connector settings (not the localhost URL).

See [Connect from ChatGPT](https://developers.openai.com/apps-sdk/deploy/connect-chatgpt) and [Submit your app](https://developers.openai.com/apps-sdk/deploy/submission).

## Validate before submit

```bash
./scripts/validate-chatgpt-submission.sh
cd src-tauri && cargo test chatgpt_submission -- --nocapture
```

The validator checks:

- JSON schema fields (`test_cases` ≥ 5, `negative_test_cases` ≥ 3)
- Subtitle length ≤ 30 characters
- All submitted tools marked `readOnlyHint: true`
- Rust alignment test: every submitted tool exists in the live MCP server

## Dashboard submission checklist

Copy from `chatgpt-app-submission.json`:

- **App name / subtitle / description** → `app_info`
- **Category** → `DEVELOPER_TOOLS`
- **Tool annotations + justifications** → `tools`
- **Positive prompts** → `test_cases`
- **Negative prompts** → `negative_test_cases`
- **Privacy summary** → `connector.privacy_summary`

Also prepare (not in JSON import):

- App logo and screenshots
- Privacy policy URL
- Company / support URL
- MCP server URL (tunnel HTTPS endpoint for review)

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Connector cannot reach MCP | Ensure AgentDeck is running; verify tunnel client is connected |
| `tools/list` empty | Restart AgentDeck; confirm `:7823` is not blocked |
| Write tool requested | Expected in dev mode only; read-only profile excludes write tools |
| Unsafe URL for localhost | Use Secure MCP Tunnel, not raw `127.0.0.1` in public connector URL |