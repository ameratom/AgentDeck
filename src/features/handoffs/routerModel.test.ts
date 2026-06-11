import { describe, expect, it } from "vitest";
import { evaluateRouter } from "./routerModel";
import type { ProviderAdapterStatus, RouterRule } from "../../lib/types";

const provider = (
  id: string,
  credentialStatus: ProviderAdapterStatus["credentialStatus"] = "not-required",
): ProviderAdapterStatus => ({
  id,
  name: id,
  kind: "openai-compatible",
  baseUrl: "http://localhost:1234/v1",
  authMode: credentialStatus === "not-required" ? "none" : "bearer-key",
  credentialStatus,
  health: { name: id, endpoint: "", available: true, detail: "" },
  models: [{ id: `${id}-model`, ownedBy: null }],
  capabilities: ["chat"],
});

const rules: RouterRule[] = [
  {
    id: "rule:code-review-grok",
    priority: 10,
    matchRules: { keywords: ["code review"] },
    route: { providerId: "xai", modelId: "grok-model" },
    warnLargeTask: false,
  },
  {
    id: "rule:local-fallback",
    priority: 100,
    matchRules: {},
    route: { providerId: "lm-studio", modelId: "" },
    warnLargeTask: false,
  },
];

describe("router model", () => {
  it("routes code review tasks to Grok", () => {
    const result = evaluateRouter({
      task: "Please run a code review on the auth module",
      context: "",
      sourceAgentId: "agent:codex",
      providers: [provider("lm-studio"), provider("xai", "keychain")],
      rules,
    });

    expect(result.providerId).toBe("xai");
    expect(result.modelId).toBe("grok-model");
  });

  it("warns on large tasks when configured", () => {
    const largeRules: RouterRule[] = [
      {
        id: "rule:large-task-warn",
        priority: 5,
        matchRules: { taskSizeGt: 20 },
        route: { providerId: "lm-studio", modelId: "" },
        warnLargeTask: true,
      },
      ...rules,
    ];

    const result = evaluateRouter({
      task: "x".repeat(30),
      context: "",
      sourceAgentId: "agent:hermes",
      providers: [provider("lm-studio")],
      rules: largeRules,
    });

    expect(result.warning).toContain("30 characters");
  });
});