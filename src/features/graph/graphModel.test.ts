import { describe, expect, it } from "vitest";
import { buildGraph } from "./graphModel";
import type { DiscoveredEntity } from "../../lib/types";

function entity(
  id: string,
  entityType: string,
  name: string,
  metadata: Record<string, string> = {},
): DiscoveredEntity {
  return {
    id,
    entityType,
    name,
    status: "available",
    source: "test",
    metadata,
  };
}

describe("graph model", () => {
  it("maps entities and their command/config relationships", () => {
    const entities = [
      entity("agent:codex", "agent", "Codex", { command: "codex" }),
      entity("tool:codex", "tool", "codex"),
      entity("config:codex", "config", "Codex"),
    ];

    const graph = buildGraph(entities);

    expect(graph.nodes).toHaveLength(3);
    expect(graph.edges.map((item) => item.label)).toEqual([
      "uses",
      "configured by",
    ]);
  });

  it("uses stable node positions for the same input", () => {
    const entities = [
      entity("agent:grok", "agent", "Grok"),
      entity("agent:hermes", "agent", "Hermes"),
      entity("process:grok", "process", "grok"),
      entity("tool:hermes", "tool", "hermes"),
    ];

    const graph = buildGraph(entities);

    expect(graph.nodes).toEqual(buildGraph(entities).nodes);
    expect(graph.edges.map((item) => item.label)).toContain("runs as");
  });

  it("connects Grok to its xAI provider", () => {
    const entities = [
      entity("agent:grok", "agent", "Grok", { providerId: "xai" }),
      entity("provider:xai:123", "provider", "xAI", { providerId: "xai" }),
    ];

    const graph = buildGraph(entities);

    expect(graph.edges).toContainEqual(
      expect.objectContaining({
        source: "provider:xai:123",
        target: "agent:grok",
        label: "powers",
      }),
    );
  });
});
