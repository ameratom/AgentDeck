# AgentDeck v0.1.6 — Phase 14 prep

Post-ChatGPT submission hardening: MCP schemas, Hermes reliability, and Activity → Handoffs navigation.

## Highlights

- **MCP output schemas** — `outputSchema` on `tools/list` and `structuredContent` on tool results for ChatGPT / Apps SDK compliance
- **Write-tool input schemas** — Deferred MCP tools (`dispatch_handoff`, `execute_skill`, `toggle_mcp_server`) now document ID patterns, examples, and approval token formats in developer mode
- **Hermes patch parsing** — Composer bridge accepts plain fences, inline hunks, and `diff --git` responses
- **Activity links** — Audit rows for `handoff.*` actions expose **View run** when a stored handoff run is linked by `audit_ref`
- **Overnight queue** — Expanded `tasks/overnight.queue.json` with four ALLOW tasks for supervised dry-runs
- **Outbound webhooks** — Settings UI for signed webhook endpoints; auto-dispatch on handoff and skill events
- **Router auto-apply** — Settings toggle to apply matching router rules automatically in Chat and Handoffs
- **Release scripts** — `pnpm tauri:build`, `pnpm notarize:preflight`, `pnpm notarize`

## ChatGPT app (v1.0.0)

Submission remains in **review**. This release does not change the published read-only HTTP MCP contract (`read_only_v1_1`, 10 tools).

When approved, publish from [platform.openai.com/apps-manage](https://platform.openai.com/apps-manage). Keep AgentDeck and the Secure MCP Tunnel running during review.

## Verify

```bash
pnpm verify
./scripts/smoke-chatgpt-tunnel.sh
AGENTDECK_COMPOSER_BRIDGE=dry-run pnpm hermes:overnight
```

## Install (when built)

1. Download `AgentDeck_0.1.6_aarch64.dmg` from GitHub Releases
2. Open the DMG and drag **AgentDeck** to Applications
3. MCP listens on `http://127.0.0.1:7823/mcp`

Notarization requires local `scripts/notarize.local.env` — run `./scripts/notarize-macos.sh` before publishing the DMG.