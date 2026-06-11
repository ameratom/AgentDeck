# AgentDeck Architecture

## Summary

AgentDeck is a local-first macOS app composed of:

1. Tauri desktop shell.
2. React UI.
3. Rust local control plane.
4. SQLite database.
5. Provider adapters.
6. Agent adapters.
7. MCP inspector/server.
8. Visual graph.

## Architecture diagram

```text
┌─────────────────────────────────────────────────────────┐
│                      AgentDeck UI                       │
│  React + TypeScript                                     │
│                                                         │
│  ┌─────────────┐  ┌────────────┐  ┌──────────────────┐ │
│  │ Chat View   │  │ Graph View │  │ Settings/Status  │ │
│  └──────┬──────┘  └─────┬──────┘  └────────┬─────────┘ │
└─────────┼───────────────┼──────────────────┼───────────┘
          │ Tauri invoke  │                  │
┌─────────▼───────────────▼──────────────────▼───────────┐
│                    Rust Control Plane                   │
│                                                         │
│  ┌─────────────┐  ┌────────────┐  ┌──────────────────┐ │
│  │ Discovery   │  │ Router     │  │ Permission Engine│ │
│  └──────┬──────┘  └─────┬──────┘  └────────┬─────────┘ │
│         │               │                  │           │
│  ┌──────▼──────┐  ┌─────▼──────┐  ┌────────▼─────────┐ │
│  │ MCP Inspect │  │ Providers  │  │ Audit/SQLite     │ │
│  └──────┬──────┘  └─────┬──────┘  └──────────────────┘ │
└─────────┼───────────────┼──────────────────────────────┘
          │               │
┌─────────▼───────┐ ┌─────▼──────────────────────────────┐
│ Local agents    │ │ Model providers                     │
│ Codex           │ │ LM Studio / Gemma                   │
│ Claude Code     │ │ OpenAI-compatible                   │
│ Grok            │ │ xAI / Grok                          │
│ Hermes          │ │ xAI / Grok                          │
│ OpenClaw        │ │ Anthropic optional                  │
└─────────────────┘ └────────────────────────────────────┘
```

## Principle

AgentDeck is not a mega-agent. It is a switchboard, dashboard, and policy layer.

Agents should communicate through AgentDeck adapters and logged handoffs, not through hidden peer-to-peer shell hacks.

## Data flow

### Environment scan

1. UI calls `scan_environment`.
2. Rust scans:
   - processes
   - known commands
   - config files
   - MCP configs
   - local providers
   - xAI-backed Grok readiness when credentials are present
3. Rust returns normalized entities and connections.
4. UI renders status table and graph.
5. SQLite stores snapshot and audit event.

### Local chat

1. User selects LM Studio provider and model.
2. UI sends message to `send_chat_message`.
3. Rust calls OpenAI-compatible endpoint.
4. Response is stored in SQLite.
5. UI displays assistant message.

### Handoff

1. User requests handoff.
2. AgentDeck builds a handoff preview.
3. User approves.
4. Adapter sends prompt/task to target.
5. Run is logged.
6. Result is attached to thread and graph edge activity.

## Adapter types

### Provider adapter

Used for model APIs:

- LM Studio
- OpenAI-compatible
- xAI/Grok
- Anthropic

### Agent adapter

Used for tool/agent runtimes:

- Codex
- Claude Code
- Grok
- Hermes
- OpenClaw

Grok is treated as a first-class source agent tied to xAI readiness, so it remains visible when xAI credentials exist and can fall back to a degraded state when the service is temporarily unavailable.

### MCP adapter

Used for MCP server inventory, validation, and later AgentDeck’s own MCP server.

### Webhook adapter

Used for inbound/outbound event integration.

## Error philosophy

Errors should be useful and local:

- `LM Studio server is not reachable at localhost:1234`
- `Claude CLI not found in PATH`
- `MCP config found but command executable is missing`
- `OpenClaw process is running but gateway status command failed`

Never display raw stack traces in the main UI.
