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
- Encrypted local provider secret storage.
- Explicit one-time import from legacy macOS Keychain entries.

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

- [x] Expose read-only tools over stdio and loopback HTTP.
- [x] Add action tools behind approval and permission checks.
- [x] Complete end-to-end validation with Claude Code and Codex.

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

## Phase 10

External connectors and routing polish.

Tasks:

- [x] Grok MCP bridge (`grok-mcp.env`) for shell launchers
- [x] Filesystem MCP launcher (project-scoped)
- [x] Git MCP launcher (read-only MVP)
- [x] Connector templates for Claude Code and Codex
- [x] Handoff router rules UI in Settings
- [x] Router suggestions in Handoffs view
- [x] v0.1.2 signed + notarized release
- [x] ChatGPT submission package (`chatgpt-app-submission.json`, tunnel scripts, validator)

## Phase 11

Project workspaces and release hardening.

Tasks:

- [x] Local project registry with deterministic path-based IDs
- [x] Active project selection and safe removal
- [x] Audited project mutations and redacted local-data exports
- [x] Projects UI with explicit global-scan boundary
- [x] Scope project config discovery, graph context, chat, and handoffs to the active project
- [x] Add per-project MCP and connector settings with validated export profiles
- [x] Complete the v0.1.3 release checklist and notarized build

## Phase 12

Project-aware onboarding and Claude Code MCP serve export.

Tasks:

- [x] Register a project workspace during first-run onboarding
- [x] Export project MCP connector profile during onboarding (AgentDeck HTTP, filesystem, git, Claude Code serve)
- [x] Include `claude mcp serve` in per-project Claude/Codex connector exports
- [x] Always include AgentDeck HTTP MCP in connector export profiles

## Phase 13

Grok-first integration, ChatGPT v1.1 research tools, and ChatGPT test seam fixes.

Tasks:

- [x] Resolve `get_run` by `runId`, `auditId`, or `conversationId`; enrich audit rows with `runId`
- [x] OR-friendly audit search tokenization
- [x] Enriched macOS PATH discovery for GUI app scans
- [x] Export `grok-mcp` and `agentdeck-xai-research-mcp` in project connector profiles
- [x] Grok-default handoff routing fallback and onboarding defaults
- [x] Proxy read-only xAI Research tools through AgentDeck HTTP MCP
- [x] ChatGPT submission package v1.1 (`read_only_v1_1`)
