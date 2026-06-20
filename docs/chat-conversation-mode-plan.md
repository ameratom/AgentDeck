# Chat Conversation Mode — Implementation Plan

**Status:** Implemented (2026-06-16)  
**Date:** 2026-06-16  
**Goal:** Make AgentDeck Chat feel like Grok / ChatGPT: conversation only, latest reply always above the composer, plus Clear chat.

---

## Problem (from current UI)

1. **Too much chrome** — Phase header, provider toolbar, status strip, router bar, message counts, and credential labels compete with the conversation.
2. **Latest reply is not visible** — With 50+ stored messages, the scroll region grows with the page instead of staying height-locked. The composer is pinned in markup but the message list expands the workspace, so Grok’s answer sits far above the input (user must hunt for it).
3. **No Clear chat** — No way to reset the visible thread; old test messages accumulate.
4. **Noisy message cards** — `USER` / `ASSISTANT` labels and timestamps dominate; empty-looking bubbles appear when content is whitespace-only or collapsed.

Backend send/stream/save is working; this is a **layout + presentation** problem.

---

## Design Target

### Visible (conversation mode)

| Zone | Content |
|------|---------|
| **Scroll area** | Message bubbles only — user right, assistant left (Grok/ChatGPT pattern). Subtle timestamp under each bubble (e.g. `6:12 PM`). |
| **Composer (pinned bottom)** | Textarea, Send, Stop (while streaming), **Clear** |

### Hidden (accessible, not on-screen)

Move to a **gear menu** (top-right of chat pane, single icon):

- Provider + model selects
- Load models
- AgentDeck tools toggle (xAI)
- Open Providers (when credentials missing)
- Router suggestion (collapsed into menu or toast — never blocks the thread)

Remove from chat surface:

- Phase 4 / Unified Chat header block
- Status strip (merge errors into inline composer hint or brief toast)
- “54 stored messages” counters
- Provider health / catalog labels
- Router suggestion bar (default: off-screen; optional setting to show)

### Layout rules (Grok-like)

```
┌─────────────────────────────────────────────┐
│  [optional gear]                     [clear]│  ← 40px top bar, minimal
├─────────────────────────────────────────────┤
│                                             │
│     (older messages scroll away)            │
│                                             │
│  ┌──────────────────────┐                   │
│  │ assistant bubble     │  6:12 PM          │
│  └──────────────────────┘                   │
│                   ┌──────────────────────┐  │
│                   │ user bubble          │  │
│                   └──────────────────────┘  │
│  ┌──────────────────────┐                   │
│  │ latest assistant     │  ← always last    │
│  └──────────────────────┘    bubble above  │
│                             composer        │
├─────────────────────────────────────────────┤
│  [ message input........................ ]  │
│                          [Clear] [Send]     │
└─────────────────────────────────────────────┘
```

- Main column: `height: 100%` of app content pane (not `100vh` — exclude sidebar).
- Grid: `auto | 1fr | auto` (thin top bar, scroll messages, composer).
- `message-list`: `overflow-y: auto`, `min-height: 0`, `flex: 1`.
- **Auto-scroll** to bottom on: new message, stream token, load, clear.
- Filter render: skip messages where `content.trim() === ""`.

---

## Clear Chat

### Backend (new)

**Command:** `clear_chat_messages(conversation_id: String) -> Result<(), String>`

- `DELETE FROM chat_messages WHERE conversation_id = ?`
- Audit: `chat.clear` with conversation id
- Reuse `validate_identifier` for conversation id

**Files:**

- `src-tauri/src/commands/chat.rs` — command + test
- `src-tauri/src/lib.rs` — register command
- `src/lib/invoke.ts` — `clearChatMessages(conversationId)`
- `src/lib/types.ts` — no new types

### Frontend

- **Clear** button in composer foot (secondary, left of Send).
- Confirm dialog: “Clear this conversation? This cannot be undone.”
- On success: `setMessages([])`, reset status, keep provider/model selection.

---

## Implementation PRs (ordered)

### PR 1 — Height-locked conversation shell (layout fix)

**Why first:** Fixes “reply not above composer” without waiting on polish.

| File | Change |
|------|--------|
| `src/styles/globals.css` | `.app-shell > .chat-workspace`: `height: 100vh; min-height: 0; overflow: hidden`. Replace `chat-workspace--compact` `100vh` with `height: 100%`. Add `.chat-conversation-mode` grid. |
| `src/features/chat/ChatView.tsx` | Add wrapper class `chat-conversation-mode`; remove outer header from default render (or gate with `conversationMode` constant `true`). |
| `src/features/chat/ChatView.tsx` | Filter `renderedMessages` with `content.trim() !== ""`. |
| `src/features/chat/ChatView.tsx` | Strengthen scroll: `requestAnimationFrame` + `scrollTop = scrollHeight` on `messages`, `streamingContent`. |

**Verify:** Send message → assistant bubble appears immediately above textarea without manual scroll.

---

### PR 2 — Grok-style bubbles + minimal chrome

| File | Change |
|------|--------|
| `src/styles/globals.css` | New `.chat-bubble`, `.chat-bubble--user`, `.chat-bubble--assistant`, `.chat-bubble-time`. Remove loud `USER`/`ASSISTANT` headers from default style. |
| `src/features/chat/ChatView.tsx` | Render bubbles: no role `<strong>`, timestamp below content, `white-space: pre-wrap`. |
| `src/features/chat/ChatView.tsx` | Top bar: gear button → `<details>` or popover with provider/model/tools (existing controls relocated). |
| `src/features/chat/ChatView.tsx` | Hide status strip; surface `sendBlockReason` / errors as composer `aria-describedby` or small text under input. |

**Verify:** Screenshot comparison — only bubbles + input visible at default zoom.

---

### PR 3 — Clear chat

| File | Change |
|------|--------|
| `src-tauri/src/commands/chat.rs` | `clear_chat_messages` |
| `src/lib/invoke.ts` | wrapper |
| `src/features/chat/ChatView.tsx` | Clear button + confirm + invoke |

**Verify:** Clear → empty state → send new message → single pair visible above composer.

---

## Empty state

When no messages:

```
        Start a conversation
   Messages are saved locally on this Mac.
```

No provider instructions in the main view (gear menu covers setup).

---

## Accessibility

- Gear menu and Clear: keyboard reachable, `aria-label`s.
- Composer: keep `aria-label="Message"`, link block reason via `aria-describedby`.
- After Clear: focus returns to textarea.
- After send: `aria-live="polite"` on message list for new assistant content.

---

## Out of scope (this plan)

- Changing sidebar navigation or removing “Read-only mode” footer (app shell, not chat).
- Multi-conversation tabs / history sidebar (future).
- Paper mockup (optional; layout is simple enough to implement directly in Qt/React per existing compact redesign pattern).

---

## Acceptance criteria

1. Opening Chat shows **only** the message thread and composer (plus unobtrusive gear/clear).
2. After Send completes, the **latest assistant reply is visible directly above the input** without scrolling.
3. **Clear** removes all messages for the current conversation (project-scoped or global) and resets the view.
4. Timestamps are subtle; role labels are not shown in the default bubble UI.
5. Provider/model remain configurable via gear menu; last selection still persisted via `chat_preferences`.

---

## Estimated effort

| PR | Effort |
|----|--------|
| PR 1 Layout | ~1–2 h |
| PR 2 Bubbles + chrome | ~2–3 h |
| PR 3 Clear | ~1 h |

**Total:** ~4–6 h implementation + manual QA in dev app.