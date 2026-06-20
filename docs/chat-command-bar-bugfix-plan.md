# Chat Command Bar — Bugfix Plan

**Status:** Implemented (2026-06-16)  
**Date:** 2026-06-16  
**Scope:** Fix Clear chat, restore multi-provider chat (Anthropic / Codex / Claude Code), darken command bar surface.

---

## TL;DR

| Issue | Likely root cause | Fix |
|-------|-------------------|-----|
| **Clear chat button does nothing** | `window.confirm()` is unreliable in the Tauri WebView; click appears to no-op when confirm never resolves true | Replace with an in-app confirm dialog; verify DB delete + UI reset |
| **Only xAI/Grok chat works** | Chat streaming always hits `/chat/completions`; Codex/OpenAI official use `/responses`. Send-time `verify_provider_model` re-fetches live models and rejects static/fallback picks | Route Codex/OpenAI official through Responses API; relax verify for static/fallback catalogs |
| **Command bar too light** | `--cmd-surface: #2c2c2c` | Change to `#212121` |

---

## 1. Clear chat does not clear

### Symptoms

- **Clear chat** in the top bar is correctly placed and enabled when messages exist.
- Clicking it leaves the thread unchanged (user reports no visible effect).

### Investigation

Backend path is implemented and registered:

- `clear_chat_messages` in `src-tauri/src/commands/chat.rs` — `DELETE FROM chat_messages WHERE conversation_id = ?1`
- Frontend `clearConversation()` in `ChatView.tsx` calls `clearChatMessages(conversationId)` then `setMessages([])`

The gate before any work runs:

```ts
if (!window.confirm("Clear this conversation? This cannot be undone.")) {
  return;
}
```

**Primary hypothesis:** `window.confirm` does not present a usable native dialog in the Tauri 2 WKWebView on macOS (returns `false` immediately or never shows). The handler exits before invoking Rust, so the button appears broken.

Secondary checks (lower probability):

- `conversationId` mismatch between load and clear — unlikely; both use the same `project`-derived ID in `ChatView.tsx`.
- Silent invoke failure — would set `status` to `Clear failed: …` but user may not notice if hint is empty.

### Proposed fix

**A. In-app confirm (preferred — no new dependency)**

1. Add local state: `clearConfirmOpen: boolean`.
2. **Clear chat** click → open a small modal/inline sheet (reuse existing `role="dialog"` + backdrop patterns from `OnboardingView.tsx` / drawers).
3. Modal copy: “Clear this conversation? This cannot be undone.” with **Cancel** / **Clear** buttons.
4. **Clear** runs existing `clearConversation()` body **without** `window.confirm`.
5. On success: close modal, clear messages, show brief status (“Conversation cleared”) in `cmdbar-hint` or topbar toast line.
6. On failure: keep modal open or close with error in `composerHint` / `status`.

**B. Hardening**

- After `clearChatMessages`, optionally `loadChatMessages(conversationId)` and assert `[]` before updating UI (catches DB/ID bugs).
- Keep **Clear conversation** in the `+` menu wired to the same confirm flow (not a silent delete).
- Add a Rust unit/integration test: insert messages → `clear_chat_messages` → `load_messages` returns empty.

### Files

| File | Change |
|------|--------|
| `src/features/chat/ChatView.tsx` | Confirm modal state + markup; remove `window.confirm` |
| `src/styles/globals.css` | `.chat-clear-confirm-*` modal styles |
| `src-tauri/src/commands/chat.rs` | Optional test for clear round-trip |

### Acceptance

- [ ] Click **Clear chat** → in-app confirm appears (no reliance on browser `confirm`).
- [ ] Confirm → thread empties immediately; reload app → thread still empty for that conversation.
- [ ] Cancel → thread unchanged.
- [ ] `+` → Clear conversation uses the same confirm path.

---

## 2. Multi-provider chat broken (only Grok works)

### Symptoms

- User can select Anthropic, Codex, Claude Code (or similar) in Settings, but sends fail or never return useful replies.
- xAI/Grok continues to work.

### Investigation

Chat send path (`stream_chat_message` in `chat.rs`):

1. `validate_chat_request`
2. **`verify_provider_model`** — live `fetch_provider_models` + require exact `model_id` match
3. `chat_providers::stream_provider_chat` by adapter

#### Bug A — Codex / OpenAI official use wrong HTTP API for chat

Handoffs already branch correctly in `providers.rs`:

```rust
if uses_openai_responses_api(definition, base_url) {
    return dispatch_openai_responses(...);  // POST /responses
}
// else POST /chat/completions
```

`uses_openai_responses_api` is true for `codex` and `openai-compatible` when base URL is `https://api.openai.com/v1`.

**Chat streaming** (`chat_providers.rs::stream_openai_compatible`) **always** posts to `/chat/completions` with `stream: true`. Codex curated models (`gpt-5.4`, `codex-mini-latest`, etc.) are oriented around the Responses API, not Chat Completions — requests fail or return errors.

**This alone explains Codex (and likely OpenAI official) chat failure while Grok (x.ai `/chat/completions`) works.**

#### Bug B — `verify_provider_model` too strict at send time

`verify_provider_model` re-fetches models from the network on every send. UI may show models from:

- `catalogSource: "static"` (Codex defaults)
- `catalogSource: "fallback"` (Anthropic when `/models` fails)

If live fetch fails or returns a different ID set than the UI dropdown, send is blocked with:

> `{Provider} did not verify model {model_id}. Reload models before dispatch.`

Frontend `providerHasDispatchableModels` allows static/fallback catalogs to enable Send, but Rust verify does not — **UI says sendable, backend rejects**.

#### Bug C — Anthropic streaming (secondary)

Adapter and endpoint exist (`POST /v1/messages`, `stream: true`). Failures may be:

- Verify rejection (Bug B) before stream starts
- Credential missing (`ANTHROPIC_API_KEY` / stored slot)
- SSE event parsing missing newer event shapes (audit against Anthropic streaming docs if verify passes but stream is empty)

#### Bug D — Claude Code (CLI adapter)

Uses `claude -p` subprocess, not HTTP. Verify requires `claude` on PATH (`fetch_claude_code_models`). Works only when CLI is installed; errors should surface clearly in `cmdbar-hint`.

Router auto-apply is **disabled** in Chat (`shouldAutoApplyRouterSuggestion(false, …)`), so router hijacking is not the cause.

### Proposed fix

**PR 1 — Align OpenAI official / Codex chat with Responses API**

1. In `chat_providers.rs`, branch `ProviderAdapter::OpenAiCompatible` like handoffs:
   - If `uses_openai_responses_api(definition, base_url)` → new `complete_openai_responses_chat` (non-streaming v1) **or** `stream_openai_responses` if streaming endpoint is available.
2. **v1 recommendation:** non-streaming Responses API for Codex/OpenAI official (collect full text, emit single `token` events or chunked fake stream for UX parity). Simpler, matches handoff code in `dispatch_openai_responses`.
3. Reuse `extract_responses_text`, `ResponsesRequest` shapes from `providers.rs` (extract shared helpers to avoid duplication).

**PR 2 — Fix `verify_provider_model` for UI-consistent catalogs**

1. Extend verify to accept models from the same sources as `status_for_definition`:
   - Union live fetch with `codex_static_models()` / `anthropic_fallback_models()` when provider id matches.
   - For `ClaudeCode` adapter: verify against `["claude-code"]` without HTTP.
2. Alternatively (lighter): pass `catalog_source` from frontend or skip verify when model was chosen from a recent `check_provider_adapter` snapshot — prefer Rust-side union to avoid trust in client.

**PR 3 — Anthropic hardening**

1. After PR 2, manually test Anthropic stream with stored key.
2. If stream returns empty: extend `stream_anthropic` SSE parser for additional `event_type` values; add Rust test with fixture SSE lines.

**PR 4 — Frontend error surfacing**

1. Ensure `ChatView` shows `Chat failed: …` / verify errors in `cmdbar-hint` (already partially wired).
2. When user switches provider, auto-run `refreshProviderModels` and reset model if invalid (existing effect — verify it runs on provider change).

### Files

| File | Change |
|------|--------|
| `src-tauri/src/commands/chat_providers.rs` | Responses API branch for codex/openai-compatible |
| `src-tauri/src/commands/providers.rs` | Shared responses helpers; fix `verify_provider_model` |
| `src-tauri/src/commands/chat.rs` | Optional: pass catalog hint into verify |
| `src/features/chat/ChatView.tsx` | Surface verify/stream errors clearly |
| `src-tauri/src/commands/chat.rs` / `chat_providers.rs` | Tests per adapter |

### Acceptance

- [ ] **Anthropic:** select provider + model → send → streamed reply (with valid API key).
- [ ] **Codex:** select Codex + e.g. `gpt-5.4` → send → reply (OpenAI key with Codex access).
- [ ] **Claude Code:** with `claude` on PATH → send → subprocess reply; without CLI → clear error in hint.
- [ ] **xAI:** regression — still streams.
- [ ] UI model dropdown choices are sendable without “did not verify model” for static/fallback entries.
- [ ] `pnpm test` + `cargo test` pass.

### Verification commands

```bash
cd /Users/claudemccready/Desktop/Scripts/Codex/AgentDeck
pnpm vitest run src/features/chat/chatModel.test.ts
cargo test --manifest-path src-tauri/Cargo.toml chat::
AGENTDECK_DEV_SHOW_DOCK=1 pnpm tauri dev
```

Manual matrix in dev app:

| Provider | Model | Expect |
|----------|-------|--------|
| xAI | grok-* | Stream OK |
| Anthropic | claude-sonnet-4-6 | Stream OK |
| Codex | gpt-5.4 | Response OK (v1 non-stream acceptable) |
| Claude Code | claude-code | CLI output OK |

---

## 3. Command bar color — `#212121`

### Change

In `src/styles/globals.css` `:root`:

```css
--cmd-surface: #212121;  /* was #2c2c2c */
```

No other token changes required; pill reads slightly darker against `#0a0f14` composer area.

### Acceptance

- [ ] Command bar background matches `#212121` visually.
- [ ] Contrast of placeholder (`#8e8e8e`) and control labels remains readable.

---

## 4. Implementation order

```
1. CSS: --cmd-surface → #212121                    (5 min, independent)
2. Clear chat: in-app confirm modal                (frontend, ~1 PR)
3. verify_provider_model union + ClaudeCode path   (Rust, ~1 PR)
4. Codex/OpenAI Responses API in chat_providers    (Rust, ~1 PR)
5. Anthropic stream audit + tests if still failing (Rust, optional)
6. Full manual provider matrix + regression        (verification)
```

Recommended stack:

- **PR 1:** UI — clear confirm + darker bar (user-visible wins fast)
- **PR 2:** Rust — verify_provider_model + Responses API chat path
- **PR 3:** Anthropic/stream tests + error copy polish

---

## 5. Non-goals

- Changing router rules or re-enabling auto-apply in Chat.
- Persisting effort level to backend (still localStorage-only).
- Replacing `window.confirm` globally in Providers/Projects/Settings (separate hygiene task).

---

## 6. Risks

| Risk | Mitigation |
|------|------------|
| Responses API non-streaming feels less “live” for Codex | Emit progressive UI via chunked token events from full response, or add streaming Responses when documented |
| Anthropic model IDs drift from fallback list | Prefer live `/models` when available; keep fallback union in verify |
| Clear confirm modal accessibility | Focus trap, `aria-modal`, Esc to cancel, initial focus on Cancel |

---

## 7. References

- Command bar implementation: `src/features/chat/cmdbar/`, `docs/chat-conversation-mode-plan.md`
- Handoff Responses path: `src-tauri/src/commands/providers.rs` (`dispatch_openai_responses`, `uses_openai_responses_api`)
- Chat stream entry: `src-tauri/src/commands/chat.rs` (`stream_chat_message`)
- Provider adapters: `src-tauri/src/commands/chat_providers.rs`