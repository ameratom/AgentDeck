import { describe, expect, it } from "vitest";
import {
  buildApprovalRecord,
  buildHandoffRequest,
  recentOutput,
  resolvePreferredHandoffModel,
  resolveSuggestedHandoffModel,
  selectActiveScan,
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
  catalogSource: "live",
  verifiedAvailable: true,
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

  it("prefers a newly scanned local source inventory", () => {
    const parent = {
      scannedAt: "2026-06-11T10:00:00Z",
      project: null,
      tools: [],
      providers: [],
      processes: [],
      configs: [],
      entities: [],
    };
    const local = { ...parent, scannedAt: "2026-06-11T10:01:00Z" };

    expect(selectActiveScan(local, parent)?.scannedAt).toBe(
      "2026-06-11T10:01:00Z",
    );
    expect(selectActiveScan(null, parent)?.scannedAt).toBe(
      "2026-06-11T10:00:00Z",
    );
    expect(selectActiveScan(parent, local)?.scannedAt).toBe(
      "2026-06-11T10:01:00Z",
    );
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

  it("keeps the selected model when a provider refresh is not verified", () => {
    const unverified = {
      ...provider("xai", "xAI"),
      verifiedAvailable: false,
      models: [],
    };

    expect(resolvePreferredHandoffModel(unverified, "grok-4")).toBe("grok-4");
    expect(resolvePreferredHandoffModel(unverified)).toBe("");
  });

  it("uses a suggested model only when the refreshed provider offers it", () => {
    const lmStudio = provider("lm-studio", "LM Studio");
    lmStudio.models = [
      { id: "google/gemma-3-4b", ownedBy: null },
      { id: "qwen/qwen3.5-9b", ownedBy: null },
    ];

    expect(resolveSuggestedHandoffModel(lmStudio, "google/gemma-3-4b")).toBe(
      "google/gemma-3-4b",
    );
    expect(resolveSuggestedHandoffModel(lmStudio, "removed/model")).toBe(
      "qwen/qwen3.5-9b",
    );
  });

  it("falls back to the captured error when no output exists", () => {
    const run: HandoffRun = {
      id: "run:1",
      projectId: null,
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
