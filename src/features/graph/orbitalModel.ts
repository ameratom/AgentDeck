import type { Edge, Node } from "@xyflow/react";
import type { DiscoveredEntity } from "../../lib/types";

export interface OrbitalNodeData extends Record<string, unknown> {
  entity: DiscoveredEntity;
  label: string;
  entityType: string;
  status: string;
  isCenter?: boolean;
  ring: 0 | 1 | 2;
}

export type OrbitalNode = Node<OrbitalNodeData>;

const RING_1_RADIUS = 220;
const RING_2_RADIUS = 360;
const NODE_SIZE = 72;
const RING_2_NODE_SIZE = 58;

const entityColors: Record<string, string> = {
  agent: "#59c7a9",
  provider: "#6da7dd",
  tool: "#8b91a8",
  process: "#7b6eb1",
  config: "#c0a36a",
};

interface OrbitalRelation {
  entity: DiscoveredEntity;
  label: string;
}

export function buildOrbitalGraph(
  selectedId: string | null,
  allEntities: DiscoveredEntity[],
): { nodes: OrbitalNode[]; edges: Edge[] } {
  if (!selectedId) {
    return { nodes: [], edges: [] };
  }

  const center = allEntities.find((entity) => entity.id === selectedId);
  if (!center) {
    return { nodes: [], edges: [] };
  }

  const ring1 = collectRelations(center, allEntities);
  const ring1Ids = new Set(ring1.map((relation) => relation.entity.id));
  const ring2Map = new Map<string, OrbitalRelation>();

  for (const relation of ring1) {
    for (const secondary of collectRelations(relation.entity, allEntities)) {
      if (
        secondary.entity.id === center.id ||
        ring1Ids.has(secondary.entity.id) ||
        ring2Map.has(secondary.entity.id)
      ) {
        continue;
      }
      ring2Map.set(secondary.entity.id, secondary);
    }
  }

  const ring2 = Array.from(ring2Map.values());
  const edges: Edge[] = [];

  const addEdge = (sourceId: string, targetId: string, label: string) => {
    edges.push({
      id: `${sourceId}->${targetId}:${label}`,
      source: sourceId,
      target: targetId,
      label,
      type: "smoothstep",
      animated: label === "runs as",
      style: { stroke: "#546670", strokeWidth: 2 },
      labelStyle: {
        fill: "#a0b0c0",
        fontSize: 10,
        fontWeight: 500,
      },
    });
  };

  for (const relation of ring1) {
    addEdge(center.id, relation.entity.id, relation.label);
  }

  for (const relation of ring2) {
    const parent = ring1.find((candidate) =>
      collectRelations(candidate.entity, allEntities).some(
        (secondary) => secondary.entity.id === relation.entity.id,
      ),
    );
    if (parent) {
      addEdge(parent.entity.id, relation.entity.id, relation.label);
    }
  }

  const nodes: OrbitalNode[] = [
    createOrbitalNode(center, 0, 0, 0, true),
    ...placeRing(ring1, 1, RING_1_RADIUS, NODE_SIZE),
    ...placeRing(ring2, 2, RING_2_RADIUS, RING_2_NODE_SIZE),
  ];

  return { nodes, edges };
}

function placeRing(
  relations: OrbitalRelation[],
  ring: 1 | 2,
  radius: number,
  nodeSize: number,
): OrbitalNode[] {
  const angleStep = (2 * Math.PI) / Math.max(relations.length, 1);
  const ringOffset = ring === 2 ? Math.PI / 8 : 0;

  return relations.map((relation, index) => {
    const angle = index * angleStep - Math.PI / 2 + ringOffset;
    const x = Math.cos(angle) * radius;
    const y = Math.sin(angle) * radius;
    return createOrbitalNode(relation.entity, x, y, ring, false, nodeSize);
  });
}

function createOrbitalNode(
  entity: DiscoveredEntity,
  x: number,
  y: number,
  ring: 0 | 1 | 2,
  isCenter: boolean,
  nodeSize = NODE_SIZE,
): OrbitalNode {
  const size = isCenter ? nodeSize + 28 : nodeSize;
  return {
    id: entity.id,
    position: { x, y },
    data: {
      entity,
      label: entity.name,
      entityType: entity.entityType,
      status: entity.status,
      isCenter,
      ring,
    },
    className: `orbital-node orbital-ring-${ring}`,
    style: {
      width: size,
      height: size,
      backgroundColor: entityColors[entity.entityType] ?? "#6b7280",
      border: isCenter ? "4px solid #fff" : "2px solid #fff",
      borderRadius: "9999px",
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      color: "#fff",
      fontSize: isCenter ? 14 : ring === 2 ? 10 : 11,
      fontWeight: isCenter ? 700 : 600,
      boxShadow: isCenter ? "0 0 0 12px rgba(255,255,255,0.06)" : undefined,
      opacity: ring === 2 ? 0.88 : 1,
      transition: "transform 0.45s ease, opacity 0.35s ease",
      cursor: isCenter ? "default" : "pointer",
    },
  };
}

function collectRelations(
  focus: DiscoveredEntity,
  allEntities: DiscoveredEntity[],
): OrbitalRelation[] {
  const related: OrbitalRelation[] = [];

  const addRelation = (entity: DiscoveredEntity, label: string) => {
    if (entity.id === focus.id) {
      return;
    }
    if (!related.some((relation) => relation.entity.id === entity.id)) {
      related.push({ entity, label });
    }
  };

  for (const entity of allEntities) {
    if (entity.id === focus.id) {
      continue;
    }

    if (focus.entityType === "agent") {
      if (entity.entityType === "tool") {
        const command = focus.metadata?.command?.toLowerCase();
        if (command && entity.name.toLowerCase() === command) {
          addRelation(entity, "uses");
        }
      }
      if (
        entity.entityType === "process" &&
        (entity.name.toLowerCase().includes(focus.name.toLowerCase()) ||
          focus.name.toLowerCase().includes(entity.name.toLowerCase()))
      ) {
        addRelation(entity, "runs as");
      }
      if (entity.entityType === "config" && entity.name === focus.name) {
        addRelation(entity, "configured by");
      }
      if (
        entity.entityType === "provider" &&
        focus.metadata?.providerId &&
        entity.metadata?.providerId === focus.metadata.providerId
      ) {
        addRelation(entity, "powered by");
      }
    }

    if (focus.entityType === "provider") {
      if (entity.entityType === "agent" && entity.metadata?.providerId === focus.id) {
        addRelation(entity, "powers");
      }
      if (entity.entityType === "tool") {
        addRelation(entity, "managed by");
      }
    }

    if (focus.entityType === "process") {
      if (entity.entityType === "tool") {
        addRelation(entity, "uses");
      }
      if (entity.entityType === "agent") {
        addRelation(entity, "runs");
      }
    }

    if (focus.entityType === "config") {
      if (entity.entityType === "agent" && entity.name === focus.name) {
        addRelation(entity, "configures");
      }
    }

    if (focus.entityType === "tool") {
      if (entity.entityType === "process" && entity.metadata?.command === focus.name) {
        addRelation(entity, "runs as");
      }
      if (entity.entityType === "agent" && entity.metadata?.command === focus.name) {
        addRelation(entity, "used by");
      }
    }

    if (
      (focus.entityType === "tool" && entity.entityType === "process") ||
      (focus.entityType === "process" && entity.entityType === "tool")
    ) {
      if (
        entity.metadata?.command === focus.name ||
        focus.metadata?.command === entity.name
      ) {
        addRelation(entity, "related");
      }
    }
  }

  return related;
}