# AgentDeck v0.1.4

Signed and notarized macOS release fixing ChatGPT MCP connector OAuth metadata and Streamable HTTP compatibility.

## Highlights

- **OAuth protected-resource fix** — PRM `resource` resolves from `MCP_PUBLIC_RESOURCE_URL` or HTTPS `AGENTDECK_MCP_URL` instead of hardcoded loopback
- **Streamable HTTP compliance** — MCP notifications return 202; GET/DELETE on `/mcp` return spec-compliant 405
- **Connector ergonomics** — oversized tool payloads truncated at 64 KB; missing `get_run` returns soft `isError` content
- **Smoke test** — `scripts/smoke-chatgpt-tunnel.sh` validates PRM resource alignment

## ChatGPT tunnel config

Set in `~/Library/Application Support/com.agentdeck.desktop/chatgpt-mcp-tunnel.env`:

```bash
# Direct OpenAI tunnel
export AGENTDECK_MCP_URL="http://127.0.0.1:7823/mcp"
export MCP_PUBLIC_RESOURCE_URL="https://api.openai.com/v1/tunnel/tunnel_YOUR_ID"

# Cloudflared/public origin
export AGENTDECK_MCP_URL="https://mcp.example.com/mcp"
export MCP_PUBLIC_RESOURCE_URL="https://mcp.example.com/mcp"
```

Restart AgentDeck after editing. Verify:

```bash
curl -s http://127.0.0.1:7823/.well-known/oauth-protected-resource/mcp | jq .
./scripts/smoke-chatgpt-tunnel.sh
```

## Install

1. Download `AgentDeck_0.1.4_aarch64.dmg`
2. Open the DMG and drag **AgentDeck** to Applications
3. Launch AgentDeck — MCP listens on `http://127.0.0.1:7823/mcp`