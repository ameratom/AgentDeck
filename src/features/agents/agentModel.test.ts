import { describe, expect, it } from "vitest";
import type { DiscoveredEntity, EnvironmentScan } from "../../lib/types";
import {
  agentConfigCount,
  agentPid,
  agentStatusClass,
  agentStatusLabel,
  filterAgents,
  formatAgentId,
  permissionAllowed,
} from "./agentModel";
import type { AgentPermissionMatrix } from "../../lib/types";

const agent = (
  id: string,
  name: string,
  status: string,
  metadata: Record<string, string> = {},
): DiscoveredEntity => ({
  id,
  entityType: "agent",
  name,
  status,
  source: "agent-discovery",
  metadata,
});

describe("agent model helpers", () => {
  it("filters and sorts agent entities", () => {
    const entities = [
      agent("agent:hermes", "Hermes", "available"),
      agent("agent:codex", "Codex", "running", { pid: "4242" }),
      {
        id: "provider:lmstudio",
        entityType: "provider",
        name: "LM Studio",
        status: "available",
        source: "localhost",
        metadata: {},
      },
    ];

    expect(filterAgents(entities).map((entry) => entry.id)).toEqual([
      "agent:codex",
      "agent:hermes",
    ]);
  });

  it("maps status labels and classes", () => {
    expect(agentStatusLabel("running")).toBe("Running");
    expect(agentStatusClass("running")).toBe("agent-status running");
    expect(agentStatusClass("missing")).toBe("agent-status unavailable");
  });

  it("formats agent ids and resolves permission state", () => {
    expect(formatAgentId("agent:claude-code")).toBe("claude code");
    const matrix: AgentPermissionMatrix = {
      agents: ["agent:codex"],
      actions: ["write-config"],
      permissions: [
        { agentId: "agent:codex", action: "write-config", allow: false },
      ],
    };
    expect(permissionAllowed(matrix, "agent:codex", "write-config")).toBe(false);
  });

  it("reads pid and config count metadata", () => {
    const entity = agent("agent:codex", "Codex", "running", {
      pid: "9912",
      configCount: "2",
    });

    expect(agentPid(entity)).toBe("9912");
    expect(agentConfigCount(entity)).toBe("2");
  });
});

describe("AgentsView data contract", () => {
  it("renders with mock scan data", () => {
    const scan: EnvironmentScan = {
      scannedAt: "2026-06-10T12:00:00Z",
      project: null,
      tools: [],
      providers: [],
      processes: [],
      configs: [],
      entities: [
        agent("agent:grok", "Grok", "available", { configCount: "1" }),
        agent("agent:codex", "Codex", "running", {
          pid: "1200",
          version: "codex-cli 1.0",
        }),
      ],
    };

    const cards = filterAgents(scan.entities);
    expect(cards).toHaveLength(2);
    expect(cards[0]?.name).toBe("Codex");
    expect(agentPid(cards[0]!)).toBe("1200");
    expect(agentStatusLabel(cards[1]?.status ?? "")).toBe("Available");
  });
});
