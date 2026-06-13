import { describe, expect, it } from "vitest";
import type { EnvironmentScan, ProviderAdapterStatus } from "../../lib/types";
import {
  buildConnectorExportRequest,
  buildOnboardingHandoffRequest,
  buildProjectRegistration,
  connectorExportSummary,
  grokCredentialReady,
  nextOnboardingStep,
  selectTestHandoffTarget,
  suggestConnectorDefaults,
  summarizeInventory,
} from "./onboardingModel";

const baseScan: EnvironmentScan = {
  scannedAt: "2026-06-10T12:00:00Z",
  project: null,
  tools: [
    { name: "codex", available: true, version: "1.0", path: "/usr/bin/codex", error: null },
    { name: "claude", available: true, version: "1.0", path: "/usr/bin/claude", error: null },
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
    expect(nextOnboardingStep("inventory")).toBe("project");
    expect(nextOnboardingStep("project")).toBe("grok-key");
    expect(nextOnboardingStep("test-handoff")).toBe("connectors");
    expect(nextOnboardingStep("connectors")).toBe("done");
    expect(nextOnboardingStep("done")).toBeNull();
  });

  it("summarizes present and missing inventory signals", () => {
    const summary = summarizeInventory(baseScan);

    expect(summary.agentCount).toBe(2);
    expect(summary.runningAgents).toBe(1);
    expect(summary.availableTools).toBe(2);
    expect(summary.validMcpConfigs).toBe(1);
    expect(summary.highlights.length).toBeGreaterThan(0);
    expect(summary.gaps).toContain("No project workspace registered yet");
  });

  it("suggests Claude Code serve when the Claude CLI is available", () => {
    expect(suggestConnectorDefaults(baseScan).claudeCodeServeEnabled).toBe(true);
  });

  it("builds connector export requests from onboarding defaults", () => {
    const request = buildConnectorExportRequest({
      filesystemEnabled: true,
      gitEnabled: false,
      claudeCodeServeEnabled: true,
    });
    expect(request.claudeCodeServeEnabled).toBe(true);
  });

  it("summarizes exported connector profiles", () => {
    const summary = connectorExportSummary({
      projectId: "project:test",
      projectName: "Test",
      projectPath: "/tmp/test",
      filesystemEnabled: true,
      gitEnabled: false,
      claudeCodeServeEnabled: true,
      claudeExportPath: "/tmp/claude.mcp.json",
      codexExportPath: "/tmp/codex.mcp.toml",
      claudeCodeServeExportPath: "/tmp/claude-code-serve.mcp.json",
      updatedAt: "now",
    });
    expect(summary).toContain("Claude Code MCP serve");
    expect(summary).toContain("AgentDeck HTTP MCP");
  });

  it("builds project registration payloads", () => {
    expect(buildProjectRegistration("/tmp/demo/", "Demo App")).toEqual({
      path: "/tmp/demo",
      name: "Demo App",
    });
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