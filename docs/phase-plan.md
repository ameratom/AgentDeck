# AgentDeck Phase Plan

## Phase 0

Bootstrap Tauri and preflight.

Tasks:

- Initialize repo.
- Add AGENTS.md.
- Add docs.
- Add preflight script.
- Add app shell.
- Add Rust command `run_preflight`.

## Phase 1

Discovery.

Tasks:

- Detect commands.
- Detect processes.
- Detect known config files.
- Parse TOML/JSON where safe.
- Return normalized entities.

## Phase 2

Graph.

Tasks:

- Add React Flow.
- Map entities to nodes.
- Map connections to edges.
- Add details drawer.

## Phase 3

LM Studio chat.

Tasks:

- Health check `http://localhost:1234/v1/models`.
- List models.
- Send chat completions.
- Store messages.

## Phase 4

Provider adapters.

Tasks:

- OpenAI-compatible adapter.
- xAI adapter.
- Anthropic adapter optional.
- Keychain storage.

## Phase 5

MCP inventory.

Tasks:

- Discover `.mcp.json`.
- Discover Claude/Codex/Hermes/OpenClaw MCP configs.
- Parse server definitions.
- Add risk labels.

## Phase 6

Manual handoffs.

Tasks:

- Handoff preview.
- Approval modal.
- Run record.
- Adapter dispatch.
- Result capture.

## Phase 7

AgentDeck MCP server.

Tasks:

- Expose read-only tools.
- Test with Claude Code and Codex.
- Add action tools behind approval.

## Phase 8

Plugins and skills.

Tasks:

- [x] Internal plugin registry.
- [x] Skill file loader.
- [x] Enable/disable UI.
- [x] Skill execution audit logging.

## Phase 9

Hardening.

Tasks:

- [x] Database migrations.
- [x] Redaction.
- [x] Export/delete.
- [x] Settings UI.
- [x] Crash-safe logs.
- [x] App bundle and hardened-runtime prep.
- [x] Developer ID signing and notarization.
