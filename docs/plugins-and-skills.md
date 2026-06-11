# Plugins and Skills

Phase 8 adds AgentDeck's local plugin and skill registry.

## Data files

- `data/plugins.yaml` defines integration modules, categories, descriptions, and capabilities.
- `data/skills/*.md` defines reusable workflows with YAML frontmatter and markdown instructions.

Plugin and skill IDs must be unique and contain only ASCII letters, numbers, hyphens, or underscores.

## Persistence

AgentDeck stores plugin enablement in its own SQLite database under the Tauri app data directory. It does not modify Codex, Claude Code, Hermes, OpenClaw, LM Studio, or other third-party configuration files.

Skill availability is derived from required plugin IDs. A skill is unavailable when any required plugin is missing or disabled.

## Execution logging

The `execute_skill` pipeline validates skill availability, dispatches through the
skill's resolved provider adapter, and writes:

- a `skill_execution_runs` record
- a matching `skill.execute` audit event

MCP clients can trigger the same pipeline with `agentdeck.execute_skill` when the
caller has the `execute-skill` permission. Manual handoffs remain approval-gated in
the Handoffs view.
