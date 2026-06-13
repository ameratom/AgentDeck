# AGENTS.md — AgentDeck

## Role

You are building AgentDeck, a macOS-first local control plane for AI agents, LLM providers, MCP servers, plugins, skills, projects, and handoffs.

Act as a senior software engineer. Make small, verifiable changes. Prefer working code over sweeping rewrites.

## Build folder

```text
/Users/claudemccready/Desktop/Scripts/Codex/AgentDeck
```

## Engineering rules

1. Analyze before editing.
2. Make surgical changes.
3. Keep the app local-first.
4. Do not hard-code user secrets.
5. Store provider credentials in AgentDeck's encrypted local secret store or
   request them at runtime. macOS Keychain access is allowed only for an
   explicit, user-initiated legacy import.
6. Do not execute destructive shell commands.
7. Do not auto-modify third-party agent configs until read/validate/export is complete.
8. Missing tools must be shown as unavailable, not fatal.
9. Use deterministic IDs for discovered entities where possible.
10. Log handoffs and actions to the audit log.

## Initial stack

- Tauri 2
- Rust backend
- React + TypeScript frontend
- Vite
- React Flow / `@xyflow/react`
- SQLite
- Rust `reqwest` for provider calls
- Rust `sysinfo` for process scanning
- Rust `serde`, `serde_json`, `toml`
- Rust `keyring` for secrets

## MVP priority

1. Tauri skeleton.
2. Read-only environment scan.
3. Graph view.
4. LM Studio local chat.
5. MCP config inventory.
6. Manual handoff router.
7. AgentDeck MCP server.

## Validation

Canonical verification command:

```bash
pnpm verify
```

`pnpm verify` runs typecheck, lint, frontend tests, Rust tests, and `scripts/preflight.sh`. Hermes must not mark any task complete unless `pnpm verify` passes.

Manual launch check (not part of `pnpm verify`):

```bash
pnpm tauri dev
```

If a command is missing, add it or document the deferral in `docs/verification.md`.

## Overnight autonomy (Hermes)

Execution chain: **Planner → Hermes → Composer (patch only) → `run_guarded` → repo**.

- Hermes classifies every action ALLOW / ASK_FIRST / DENY per [docs/autonomy-policy.md](docs/autonomy-policy.md).
- All shell commands in the overnight loop route through `run_guarded` in `src-tauri/src/autonomy/command_runner.rs`.
- Composer returns patch text only via `invoke_composer`; it does not execute commands or access credentials.
- Overnight queue, loop, and report format: [HERMES.md](HERMES.md).
- Composer bridge: `cursor agent --print --mode plan` via `invoke_composer` (`src-tauri/src/autonomy/composer_bridge.rs`). Requires `CURSOR_API_KEY` or `cursor agent login`.
- Unattended overnight runs require [enablement conditions](docs/verification.md#overnight-autonomy-enablement-18), including Cursor Agent auth.
