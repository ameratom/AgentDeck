# Chat Conversation Mode

**Status:** Canonical and implemented
**Updated:** 2026-06-20

AgentDeck Chat uses one conversation-focused interface. This document is the source of truth for its layout and behavior.

## Required Surface

- `chat-workspace--conversation` owns the full-height chat workspace.
- `chat-conversation-panel` contains a minimal settings/clear top bar, the message thread, and the command area.
- `chat-thread` is the only scrolling conversation region.
- Messages render as `chat-bubble` elements, with the user aligned right and assistants aligned left.
- `chat-command-area` pins `CmdBar` below the latest reply.
- Provider, model, model refresh, and provider setup controls live inside `chat-settings-menu`.
- Empty conversations show only the local-storage message and command bar.

## Command Bar

`src/features/chat/cmdbar/` owns the active composer UI:

- Auto-growing prompt input.
- Send, stop, and dictation behavior.
- Clear-conversation action.
- Agent tools, project connectors, and effort controls.
- Inline blocked/error state.

No parallel composer implementation or alternate chat layout should be added.

## Behavioral Invariants

1. The latest assistant reply remains directly above the command bar.
2. New messages and stream updates scroll the thread to the bottom.
3. Whitespace-only messages never render.
4. Clear chat uses the in-app confirmation dialog and persists deletion through `clear_chat_messages`.
5. Provider/model selections remain configurable without occupying the conversation surface.
6. Chat remains project-scoped when an active project exists.

## Verification

```bash
pnpm typecheck
pnpm vitest run src/features/chat/chatModel.test.ts
cargo test --manifest-path src-tauri/Cargo.toml chat::
pnpm verify
```

Manual QA must confirm the command bar stays pinned, the latest reply is visible, settings open from the top bar, and clear/cancel both behave correctly.
