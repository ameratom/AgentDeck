# AgentDeck — Codex Handoff

Build folder:

```text
/Users/claudemccready/Desktop/Scripts/Codex/AgentDeck
```

## Current Implementation Status (June 2026)

**Phase 1 & 2 completed.** The core Graph view is functional with the following features:

- **Orbital Graph** (1 ring) with relationship labels on edges
- Toggle between **Orbital** and **Flat** graph modes
- **Searchable entity dropdown** in the header to center the orbital view
- **Clear selection** button
- Improved node visuals with hover transitions
- Environment scanning and entity discovery working
- Split-pane layout removed in favor of cleaner header + graph design

The app successfully launches via `pnpm tauri dev` and the orbital view is the primary visualization.

---

## Mission

Build **AgentDeck**, a macOS-first local control plane for AI agents, local LLMs, MCP servers, IDE integrations, skills, plugins, webhooks, and project-specific automations.

AgentDeck should make one thing obvious:

> What is running, what is connected, what is allowed, what changed, and which agent handled which part of the work.

The first release is **observability + controlled chat routing**, not full autonomous orchestration.