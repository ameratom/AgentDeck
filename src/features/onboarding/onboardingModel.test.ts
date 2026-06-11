import { describe, expect, it } from "vitest";
import type { EnvironmentScan, ProviderAdapterStatus } from "../../lib/types";
import {
  buildOnboardingHandoffRequest,
  grokCredentialReady,
  nextOnboardingStep,
  selectTestHandoffTarget,
  summarizeInventory,
} from "./onboardingModel";

const baseScan: EnvironmentScan = {
  scannedAt: "2026-06-10T12:00:00Z",
  tools: [
    { name: "codex", available: true, version: "1.0", path: "/usr/bin/codex", error: null },
    { name: "claude", available: false, version: null, path: null, error: "missing" },
  ],
  providers: [],
  processes: [],
  configs: [
    {
      id: "config:claude",
      kind: "mcp",
      path: "/tmp/claude.json",
      exists: true,
      format: "json",
      valid: true,
      topLevelKeys: ["mcpServers"],
      error: null,
    },
  ],
  entities: [
    {
      id: "agent:grok",
      entityType: "agent",
      name: "Grok",
      status: "available",
      source: "xai",
      metadata: {},
    },
    {
      id: "agent:codex",
      entityType: "agent",
      name: "Codex",
      status: "running",
      source: "openai",
      metadata: { pid: "1234" },
    },
  ],
};

function provider(
  id: string,
  credentialStatus: ProviderAdapterStatus["credentialStatus"],
  available = true,
): ProviderAdapterStatus {
  return {
    id,
    name: id,
    kind: "cloud",
    baseUrl: "https://example.com",
    authMode: "api-key",
    credentialStatus,
    catalogSource: available ? "live" : "none",
    verifiedAvailable: available,
    health: {
      name: id,
      endpoint: "https://example.com",
      available,
      detail: "ok",
    },
    models: [{ id: `${id}-model`, ownedBy: null }],
    capabilities: [],
  };
}

describe("onboarding model", () => {
  it("advances through the onboarding step order", () => {
    expect(nextOnboardingStep("scan")).toBe("inventory");
    expect(nextOnboardingStep("test-handoff")).toBe("done");
    expect(nextOnboardingStep("done")).toBeNull();
  });

  it("summarizes present and missing inventory signals", () => {
    const summary = summarizeInventory(baseScan);

    expect(summary.agentCount).toBe(2);
    expect(summary.runningAgents).toBe(1);
    expect(summary.availableTools).toBe(1);
    expect(summary.validMcpConfigs).toBe(1);
    expect(summary.highlights.length).toBeGreaterThan(0);
    expect(summary.gaps).not.toContain("No local agents discovered yet");
  });

  it("detects when Grok credentials are ready", () => {
    expect(
      grokCredentialReady([provider("xai", "stored"), provider("lm-studio", "not-required")]),
    ).toBe(true);
    expect(grokCredentialReady([provider("xai", "missing")])).toBe(false);
  });

  it("prefers LM Studio for the onboarding handoff smoke test", () => {
    const target = selectTestHandoffTarget([
      provider("xai", "stored"),
      provider("lm-studio", "not-required"),
    ]);

    expect(target?.id).toBe("lm-studio");
  });

  it("builds a short onboarding handoff request", () => {
    const request = buildOnboardingHandoffRequest({
      sourceAgent: baseScan.entities[1],
      provider: provider("lm-studio", "not-required"),
    });

    expect(request.title).toBe("AgentDeck onboarding check");
    expect(request.targetProviderId).toBe("lm-studio");
    expect(request.approvals).toHaveLength(1);
  });
});
