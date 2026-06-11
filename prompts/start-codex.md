You are Codex working inside:

```text
/Users/claudemccready/Desktop/Scripts/Codex/AgentDeck
```

Read:

1. `CODEX_HANDOFF.md`
2. `AGENTS.md`
3. `docs/architecture.md`
4. `docs/phase-plan.md`
5. `docs/security-model.md`
6. `docs/mcp-plugin-skill-inventory.md`

Then execute **Phase 0 only**.

Phase 0 goal:

Create a working Tauri 2 + React + TypeScript + Rust skeleton for AgentDeck with a safe read-only preflight.

Requirements:

1. Use Tauri 2.
2. Use React + TypeScript + Vite.
3. Add `@xyflow/react`, but graph implementation can be a placeholder screen in Phase 0.
4. Add a Rust command named `run_preflight`.
5. Add a Rust command named `scan_environment`.
6. Add `scripts/preflight.sh`.
7. The app must launch with `pnpm tauri dev`.
8. The UI should show:
   - app title
   - preflight button
   - scan environment button
   - results JSON panel
   - navigation placeholders for Chat, Graph, Agents, MCP, Settings.
9. Do not add destructive shell commands.
10. Do not modify external Codex/Claude/Hermes/OpenClaw configs.
11. Missing tools should be reported as unavailable, not fatal.

Suggested preflight checks:

- `node --version`
- `pnpm --version`
- `rustc --version`
- `cargo --version`
- `git --version`
- `codex --version`
- `claude --version`
- `lms --version`
- `hermes --version`
- `openclaw --version`
- `curl -s http://localhost:1234/v1/models`

Implement minimal types for:

- `ToolStatus`
- `ProviderHealth`
- `EnvironmentScan`
- `DetectedProcess`
- `DetectedConfig`

Validation:

- run `pnpm typecheck`
- run `cargo test --manifest-path src-tauri/Cargo.toml`
- run `pnpm tauri dev` or at least `pnpm tauri build --debug` if dev cannot stay open.

Stop after Phase 0 and summarize exactly what was created.
