import {
  Background,
  BackgroundVariant,
  Controls,
  MarkerType,
  MiniMap,
  ReactFlow,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useMemo } from "react";
import type { DiscoveredEntity } from "../../lib/types";
import { buildGraph, type EntityNode } from "./graphModel";

interface GraphCanvasProps {
  entities: DiscoveredEntity[];
  showProcesses: boolean;
  onSelect: (entity: DiscoveredEntity) => void;
}

const nodeColors: Record<string, string> = {
  agent: "#59c7a9",
  provider: "#6da7dd",
  config: "#c0a36a",
  tool: "#8b91a8",
  process: "#7b6eb1",
};

export function GraphCanvas({
  entities,
  showProcesses,
  onSelect,
}: GraphCanvasProps) {
  const visibleEntities = useMemo(
    () =>
      showProcesses
        ? entities
        : entities.filter((entity) => entity.entityType !== "process"),
    [entities, showProcesses],
  );
  const graph = useMemo(() => buildGraph(visibleEntities), [visibleEntities]);

  return (
    <div className="graph-canvas">
      <ReactFlow<EntityNode>
        colorMode="dark"
        defaultEdgeOptions={{
          markerEnd: { type: MarkerType.ArrowClosed, color: "#546670" },
          style: { stroke: "#546670" },
        }}
        edges={graph.edges}
        fitView
        fitViewOptions={{ padding: 0.18 }}
        maxZoom={1.4}
        minZoom={0.12}
        nodes={graph.nodes}
        nodesConnectable={false}
        nodesDraggable
        onNodeClick={(_, node) => onSelect(node.data.entity)}
        proOptions={{ hideAttribution: true }}
      >
        <Background
          color="#22313a"
          gap={22}
          size={1}
          variant={BackgroundVariant.Dots}
        />
        <Controls showInteractive={false} />
        <MiniMap
          bgColor="#0b1015"
          maskColor="rgba(3, 7, 10, 0.72)"
          nodeColor={(node) =>
            nodeColors[String(node.data.entityType)] ?? "#66727a"
          }
          nodeStrokeColor="#0a0e12"
          pannable
          zoomable
        />
      </ReactFlow>
    </div>
  );
}
