import { describe, expect, it } from "vitest";
import { buildChatRequest, selectDefaultModel } from "./chatModel";
import type { ChatMessage, LocalModel } from "../../lib/types";

describe("chat model helpers", () => {
  it("prefers a chat model over an embedding model", () => {
    const models: LocalModel[] = [
      { id: "text-embedding-nomic-embed-text-v1.5", ownedBy: null },
      { id: "qwen/qwen3.5-9b", ownedBy: null },
    ];

    expect(selectDefaultModel(models)).toBe("qwen/qwen3.5-9b");
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
        "lm-studio",
        "qwen/qwen3.5-9b",
        messages,
        "  hello  ",
      ),
    ).toEqual({
      conversationId: "conversation:test",
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
