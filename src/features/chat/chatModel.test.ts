import { describe, expect, it } from "vitest";
import {
  buildChatRequest,
  resolvePreferredModel,
  resolveSuggestedChatModel,
  selectDefaultModel,
} from "./chatModel";
import type { ChatMessage, LocalModel } from "../../lib/types";

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
