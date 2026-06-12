# AgentDeck

macOS-first local control plane for AI agents, LLM providers, MCP servers, plugins, skills, and handoffs.

## Features

- Environment discovery and orbital graph
- Multi-provider streaming chat (LM Studio, xAI/Grok, Anthropic, Codex, Claude Code)
- Handoff router with approval gate
- MCP hub (stdio + HTTP on `localhost:7823`)
- Plugin and skill registry
- Menu bar tray, onboarding, audit log

## Development

```bash
pnpm install
pnpm tauri dev
```

## Release build (signed + notarized)

```bash
source scripts/notarize.local.env
pnpm tauri build
```

Artifacts:

- `src-tauri/target/release/bundle/macos/AgentDeck.app`
- `src-tauri/target/release/bundle/dmg/AgentDeck_0.1.1_aarch64.dmg`

## Docs

- [MCP server](docs/agentdeck-mcp-server.md)
- [MCP connectors](docs/mcp-connectors.md)
- [Distribution](docs/distribution.md)

## License

Private / unpublished — adjust as needed.