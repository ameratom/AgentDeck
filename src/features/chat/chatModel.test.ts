import { describe, expect, it } from "vitest";
import {
  buildChatRequest,
  describeChatSendBlock,
  formatBubbleTimestamp,
  formatMessageTimestamp,
  resolvePreferredModel,
  resolveSuggestedChatModel,
  selectDefaultModel,
  selectDefaultProvider,
  visibleChatMessages,
} from "./chatModel";
import type { ChatMessage, LocalModel, ProviderAdapterStatus } from "../../lib/types";

const baseProvider: ProviderAdapterStatus = {
  id: "xai",
  name: "xAI",
  kind: "openai-compatible",
  baseUrl: "https://api.x.ai/v1",
  authMode: "bearer-key",
  credentialStatus: "stored",
  catalogSource: "live",
  verifiedAvailable: true,
  health: {
    name: "xAI",
    endpoint: "https://api.x.ai/v1",
    available: true,
    detail: "8 models available.",
  },
  models: [{ id: "grok-4", ownedBy: null }],
  capabilities: ["models", "chat"],
};

describe("chat model helpers", () => {
  it("prefers a chat model over an embedding model", () => {
    const models: LocalModel[] = [
      { id: "text-embedding-nomic-embed-text-v1.5", ownedBy: null },
      { id: "qwen/qwen3.5-9b", ownedBy: null },
    ];

    expect(selectDefaultModel(models)).toBe("qwen/qwen3.5-9b");
    expect(resolvePreferredModel(models, "removed-cloud-model")).toBe(
      "qwen/qwen3.5-9b",
    );
  });

  it("retains a selected model while a failed refresh has no replacement", () => {
    expect(resolvePreferredModel([], "gpt-5.4")).toBe("");
    expect(
      resolvePreferredModel(
        [{ id: "gpt-5.4", ownedBy: "openai" }],
        "gpt-5.4",
      ),
    ).toBe("gpt-5.4");
  });

  it("uses a router model only when the provider offers it", () => {
    const models: LocalModel[] = [
      { id: "text-embedding-small", ownedBy: null },
      { id: "gpt-5.4", ownedBy: "openai" },
    ];

    expect(resolveSuggestedChatModel(models, "gpt-5.4")).toBe("gpt-5.4");
    expect(resolveSuggestedChatModel(models, "removed-model")).toBe("gpt-5.4");
  });

  it("formats missing timestamps as sending", () => {
    expect(formatMessageTimestamp(null)).toBe("Sending...");
    expect(formatMessageTimestamp("2026-06-15T23:50:04.931841+00:00")).not.toBe(
      "Sending...",
    );
  });

  it("formats bubble timestamps as short local times", () => {
    expect(formatBubbleTimestamp(null)).toBe("");
    expect(formatBubbleTimestamp("streaming")).toBe("");
    expect(formatBubbleTimestamp("2026-06-15T23:50:04.931841+00:00")).toMatch(
      /\d/,
    );
  });

  it("hides empty messages from the thread", () => {
    const messages: ChatMessage[] = [
      {
        id: "message:1",
        conversationId: "conversation:test",
        role: "user",
        content: "   ",
        model: "grok-4",
        createdAt: "2026-06-15T00:00:00Z",
      },
      {
        id: "message:2",
        conversationId: "conversation:test",
        role: "assistant",
        content: "Hello",
        model: "grok-4",
        createdAt: "2026-06-15T00:00:01Z",
      },
    ];

    expect(visibleChatMessages(messages)).toHaveLength(1);
    expect(visibleChatMessages(messages)[0]?.content).toBe("Hello");
  });

  it("explains why send is blocked", () => {
    expect(
      describeChatSendBlock({
        selectedProviderId: "xai",
        selectedModel: "",
        draft: "hello",
        sending: false,
        provider: baseProvider,
      }),
    ).toBe("Choose a model before sending.");
    expect(
      describeChatSendBlock({
        selectedProviderId: "xai",
        selectedModel: "grok-4",
        draft: "hello",
        sending: false,
        provider: baseProvider,
      }),
    ).toBeNull();
  });

  it("prefers subscription CLI providers when they are ready", () => {
    const cliProvider: ProviderAdapterStatus = {
      ...baseProvider,
      id: "claude-code",
      name: "Claude Pro (CLI)",
      authMode: "none",
      credentialStatus: "not-required",
      catalogSource: "static",
      models: [{ id: "claude-code", ownedBy: "claude-cli" }],
    };
    const codexProvider: ProviderAdapterStatus = {
      ...baseProvider,
      id: "codex",
      name: "Codex (ChatGPT Plus)",
      authMode: "none",
      credentialStatus: "not-required",
      catalogSource: "static",
      models: [{ id: "gpt-5.5", ownedBy: "openai-codex" }],
    };
    const lmStudio: ProviderAdapterStatus = {
      ...baseProvider,
      id: "lm-studio",
      name: "LM Studio",
      authMode: "none",
      credentialStatus: "not-required",
      catalogSource: "live",
      models: [{ id: "qwen/qwen3.5-9b", ownedBy: null }],
    };

    expect(
      selectDefaultProvider([lmStudio, codexProvider, cliProvider]),
    ).toBe("claude-code");
    expect(selectDefaultProvider([lmStudio, codexProvider])).toBe("codex");
    expect(selectDefaultProvider([lmStudio])).toBe("lm-studio");
    expect(
      selectDefaultProvider([lmStudio, codexProvider], "codex"),
    ).toBe("codex");
  });

  it("builds a local chat request with trimmed user content", () => {
    const messages: ChatMessage[] = [
      {
        id: "message:1",
        conversationId: "conversation:test",
        role: "assistant",
        content: "Ready.",
        model: "qwen/qwen3.5-9b",
        createdAt: "2026-06-08T00:00:00Z",
      },
    ];

    expect(
      buildChatRequest(
        "conversation:test",
        "project:test",
        "lm-studio",
        "qwen/qwen3.5-9b",
        messages,
        "  hello  ",
      ),
    ).toEqual({
      conversationId: "conversation:test",
      projectId: "project:test",
      enableAgentTools: false,
      providerId: "lm-studio",
      model: "qwen/qwen3.5-9b",
      messages: [
        { role: "assistant", content: "Ready." },
        { role: "user", content: "hello" },
      ],
    });
  });
});
