import { Position, type Edge, type Node } from "@xyflow/react";
import type { DiscoveredEntity } from "../../lib/types";

export interface EntityNodeData extends Record<string, unknown> {
  entity: DiscoveredEntity;
  label: string;
  entityType: string;
  status: string;
}

export type EntityNode = Node<EntityNodeData>;

const columnOrder = ["agent", "provider", "config", "tool", "process"];
const columnStarts = [0, 240, 480, 720, 1180];
const rowsPerLane = 12;

const aliases: Record<string, string[]> = {
  "agent:codex": ["codex"],
  "agent:claude-code": ["claude"],
  "agent:hermes": ["hermes"],
  "agent:openclaw": ["openclaw"],
  "agent:lm-studio": ["lm studio", "lmstudio", "lms"],
  "agent:grok": ["grok", "xai"],
  "provider:lmstudio:http-localhost-1234-v1": ["lm studio", "lmstudio", "lms"],
};

export function buildGraph(entities: DiscoveredEntity[]): {
  nodes: EntityNode[];
  edges: Edge[];
} {
  const grouped = entities.reduce<Map<string, DiscoveredEntity[]>>(
    (groups, entity) => {
      const group = groups.get(entity.entityType) ?? [];
      group.push(entity);
      groups.set(entity.entityType, group);
      return groups;
    },
    new Map(),
  );
  const nodes = columnOrder.flatMap((entityType, columnIndex) => {
    const group = [...(grouped.get(entityType) ?? [])].sort((left, right) =>
      left.name.localeCompare(right.name),
    );

    return group.map((entity, itemIndex) => {
      const lane = Math.floor(itemIndex / rowsPerLane);
      const row = itemIndex % rowsPerLane;
      return {
        id: entity.id,
        position: {
          x: columnStarts[columnIndex] + lane * 220,
          y: row * 112 + (columnIndex % 2) * 44,
        },
        data: {
          entity,
          label: entity.name,
          entityType: entity.entityType,
          status: entity.status,
        },
        className: `entity-node entity-${entity.entityType} status-${entity.status}`,
        sourcePosition: Position.Right,
        targetPosition: Position.Left,
      };
    });
  });

  return {
    nodes,
    edges: buildEdges(entities),
  };
}

function buildEdges(entities: DiscoveredEntity[]): Edge[] {
  const edges: Edge[] = [];
  const entityIds = new Set(entities.map((entity) => entity.id));
  const agentEntities = entities.filter((entity) => entity.entityType === "agent");
  const providerEntities = entities.filter((entity) => entity.entityType === "provider");
  const configs = entities.filter((entity) => entity.entityType === "config");
  const processes = entities.filter((entity) => entity.entityType === "process");
  const providerIdsByKey = new Map(
    providerEntities.map((provider) => [
      provider.metadata.providerId ?? provider.name.toLowerCase(),
      provider.id,
    ]),
  );

  for (const agent of agentEntities) {
    const command = agent.metadata.command;
    if (command && entityIds.has(`tool:${command}`)) {
      edges.push(edge(agent.id, `tool:${command}`, "uses"));
    }

    for (const config of configs.filter((config) => config.name === agent.name)) {
      edges.push(edge(agent.id, config.id, "configured by"));
    }

    for (const process of processes.filter((process) =>
      matchesAliases(process, aliases[agent.id] ?? []),
    )) {
      edges.push(edge(agent.id, process.id, "runs as"));
    }

    const providerKey = agent.metadata.providerId;
    const providerId = providerKey ? providerIdsByKey.get(providerKey) : null;
    if (providerId) {
      edges.push(edge(providerId, agent.id, "powers"));
    }
  }

  const providerId = "provider:lmstudio:http-localhost-1234-v1";
  if (entityIds.has(providerId)) {
    for (const toolId of ["tool:lms", "tool:lmstudio"]) {
      if (entityIds.has(toolId)) {
        edges.push(edge(providerId, toolId, "managed by"));
      }
    }
    for (const process of processes.filter((process) =>
      matchesAliases(process, aliases[providerId]),
    )) {
      edges.push(edge(providerId, process.id, "runs as"));
    }
  }

  return uniqueEdges(edges);
}

function matchesAliases(entity: DiscoveredEntity, entityAliases: string[]): boolean {
  const searchable = [
    entity.name,
    entity.source,
    ...Object.values(entity.metadata),
  ]
    .join(" ")
    .toLowerCase();
  return entityAliases.some((alias) => searchable.includes(alias));
}

function edge(source: string, target: string, label: string): Edge {
  return {
    id: `${source}->${target}:${label}`,
    source,
    target,
    label,
    type: "smoothstep",
    animated: label === "runs as",
    className: "entity-edge",
  };
}

function uniqueEdges(edges: Edge[]): Edge[] {
  return [...new Map(edges.map((item) => [item.id, item])).values()];
}
