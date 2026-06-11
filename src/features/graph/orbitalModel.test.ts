import { describe, expect, it } from "vitest";
import { buildOrbitalGraph } from "./orbitalModel";
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

describe("orbital model", () => {
  it("places the selected entity at ring 0", () => {
    const entities = [
      entity("agent:codex", "agent", "Codex", { command: "codex" }),
      entity("tool:codex", "tool", "codex"),
    ];

    const graph = buildOrbitalGraph("agent:codex", entities);
    const center = graph.nodes.find((node) => node.id === "agent:codex");

    expect(center?.data.ring).toBe(0);
    expect(center?.data.isCenter).toBe(true);
    expect(center?.position).toEqual({ x: 0, y: 0 });
  });

  it("places direct relations on ring 1", () => {
    const entities = [
      entity("agent:codex", "agent", "Codex", { command: "codex" }),
      entity("tool:codex", "tool", "codex"),
      entity("config:codex", "config", "Codex"),
    ];

    const graph = buildOrbitalGraph("agent:codex", entities);
    const ring1 = graph.nodes.filter((node) => node.data.ring === 1);

    expect(ring1).toHaveLength(2);
    expect(ring1.every((node) => node.position.x !== 0 || node.position.y !== 0)).toBe(
      true,
    );
  });

  it("places secondary relations on ring 2", () => {
    const entities = [
      entity("agent:codex", "agent", "Codex", { command: "codex" }),
      entity("tool:codex", "tool", "codex"),
      entity("process:runner", "process", "runner", { command: "codex" }),
    ];

    const graph = buildOrbitalGraph("agent:codex", entities);
    const ring2 = graph.nodes.filter((node) => node.data.ring === 2);

    expect(ring2).toHaveLength(1);
    expect(ring2[0]?.id).toBe("process:runner");
  });

  it("returns empty graph when selection is missing", () => {
    expect(buildOrbitalGraph(null, [])).toEqual({ nodes: [], edges: [] });
    expect(buildOrbitalGraph("missing", [entity("agent:grok", "agent", "Grok")])).toEqual({
      nodes: [],
      edges: [],
    });
  });
});