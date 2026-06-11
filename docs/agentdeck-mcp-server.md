# AgentDeck MCP Server

AgentDeck runs as a local MCP server with stdio and HTTP transports.

## Stdio

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- --mcp-server
```

## HTTP

The desktop app starts an HTTP MCP listener on `http://127.0.0.1:7823` for remote
clients such as ChatGPT.

## Tools

| Tool | Access | Description |
|------|--------|-------------|
| `agentdeck.scan_environment` | Read | Current environment scan |
| `agentdeck.get_graph` | Read | Graph snapshot |
| `agentdeck.list_agents` | Read | Agent status list |
| `agentdeck.list_mcp_servers` | Read | MCP inventory |
| `agentdeck.health_check` | Read | Preflight result |
| `agentdeck.get_run` | Read | Single handoff run |
| `agentdeck.search_audit_log` | Read | Audit search |
| `agentdeck.dispatch_handoff` | Write | Trigger a handoff |
| `agentdeck.execute_skill` | Write | Run a skill |
| `agentdeck.toggle_mcp_server` | Write | Enable/disable MCP server |

Write tools enforce the same backup/restore protocol and permission checks as the
desktop UI. The server reads the local SQLite database used by AgentDeck and does
not start external MCP servers on its own.

For Grok, Claude Code, and Codex connector setup, see [mcp-connectors.md](./mcp-connectors.md).