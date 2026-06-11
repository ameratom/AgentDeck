import { describe, expect, it } from "vitest";
import {
  buildApprovalRecord,
  buildHandoffRequest,
  recentOutput,
  selectDefaultModel,
  selectDefaultModelFromList,
  selectDefaultTargetProvider,
} from "./handoffModel";
import type { HandoffRun, ProviderAdapterStatus } from "../../lib/types";

const provider = (id: string, name: string): ProviderAdapterStatus => ({
  id,
  name,
  kind: "openai-compatible",
  baseUrl: id === "lm-studio" ? "http://localhost:1234/v1" : "https://api.example.com/v1",
  authMode: id === "lm-studio" ? "none" : "bearer-key",
  credentialStatus: id === "lm-studio" ? "not-required" : "missing",
  health: {
    name,
    endpoint: "",
    available: true,
    detail: "",
  },
  models: [{ id: `${id}/model`, ownedBy: null }],
  capabilities: ["models", "chat"],
});

describe("handoff model helpers", () => {
  it("prefers LM Studio as the default target provider", () => {
    expect(
      selectDefaultTargetProvider([
        provider("xai", "xAI"),
        provider("lm-studio", "LM Studio"),
      ])?.id,
    ).toBe("lm-studio");
  });

  it("builds a trimmed handoff request", () => {
    const request = buildHandoffRequest({
      sourceAgentId: "agent:codex",
      sourceAgentName: "Codex",
      provider: provider("lm-studio", "LM Studio"),
      modelId: "qwen/qwen3.5-9b",
      title: "  Review  ",
      task: "  Summarize  ",
      context: "  Focus  ",
      approvals: ["user-approved:test"],
    });

    expect(request.title).toBe("Review");
    expect(request.task).toBe("Summarize");
    expect(request.context).toBe("Focus");
    expect(request.approvals).toEqual(["user-approved:test"]);
  });

  it("builds an approval record with timestamp", () => {
    expect(buildApprovalRecord()).toMatch(/^user-approved:/);
  });

  it("prefers qwen chat models over gemma and embedding models", () => {
    const lmStudio = provider("lm-studio", "LM Studio");
    lmStudio.models = [
      { id: "google/gemma-3-4b", ownedBy: null },
      { id: "text-embedding-nomic-embed-text-v1.5", ownedBy: null },
      { id: "qwen/qwen3.5-9b", ownedBy: null },
    ];

    expect(selectDefaultModel(lmStudio)).toBe("qwen/qwen3.5-9b");
    expect(
      selectDefaultModelFromList([
        { id: "google/gemma-3-4b", ownedBy: null },
        { id: "qwen/qwen3.5-9b", ownedBy: null },
      ]),
    ).toBe("qwen/qwen3.5-9b");
  });

  it("falls back to the captured error when no output exists", () => {
    const run: HandoffRun = {
      id: "run:1",
      threadId: "handoff:agent:codex:lm-studio",
      sourceAgentId: "agent:codex",
      sourceAgentName: "Codex",
      targetProviderId: "lm-studio",
      targetProviderName: "LM Studio",
      targetModelId: "qwen/qwen3.5-9b",
      title: "Review",
      task: "Summarize",
      context: "Focus",
      status: "failed",
      output: "",
      error: "failed",
      approvals: ["user"],
      auditRef: null,
      createdAt: "2026-06-08T00:00:00Z",
      updatedAt: "2026-06-08T00:00:00Z",
    };

    expect(recentOutput(run)).toBe("failed");
    expect(selectDefaultModel(provider("lm-studio", "LM Studio"))).toBe("lm-studio/model");
  });
});
