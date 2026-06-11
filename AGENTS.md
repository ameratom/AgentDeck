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
5. Store provider credentials in macOS Keychain or request them at runtime.
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

After each phase, run:

```bash
pnpm typecheck
pnpm lint
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri dev
```

If a command is not configured yet, add the missing script or document why it is deferred.
