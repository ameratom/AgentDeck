import {
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ReactFlow,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useMemo } from "react";
import type { DiscoveredEntity } from "../../lib/types";
import {
  buildOrbitalGraph,
  type OrbitalNodeData,
  type OrbitalNode,
} from "./orbitalModel";

interface OrbitalGraphProps {
  entities: DiscoveredEntity[];
  selectedId: string | null;
  onSelect: (entity: DiscoveredEntity) => void;
}

export function OrbitalGraph({
  entities,
  selectedId,
  onSelect,
}: OrbitalGraphProps) {
  const { nodes, edges } = useMemo(
    () => buildOrbitalGraph(selectedId, entities),
    [selectedId, entities],
  );

  if (!selectedId || nodes.length === 0) {
    return (
      <div className="orbital-empty">
        <p>Select an entity from the list to see its orbital view.</p>
      </div>
    );
  }

  return (
    <div className="orbital-graph" data-transition={selectedId}>
      <div className="orbital-ring-guides" aria-hidden="true">
        <span className="orbital-guide orbital-guide-1" />
        <span className="orbital-guide orbital-guide-2" />
      </div>
      <ReactFlow<OrbitalNode>
        colorMode="dark"
        edges={edges}
        fitView
        fitViewOptions={{ padding: 0.4 }}
        maxZoom={1.8}
        minZoom={0.4}
        nodes={nodes}
        nodesConnectable={false}
        nodesDraggable
        onNodeClick={(_, node) => onSelect(node.data.entity)}
        proOptions={{ hideAttribution: true }}
      >
        <Background
          color="#22313a"
          gap={24}
          size={1}
          variant={BackgroundVariant.Dots}
        />
        <Controls showInteractive={false} />
        <MiniMap
          bgColor="#0b1015"
          maskColor="rgba(3, 7, 10, 0.72)"
          nodeColor={(node) =>
            (node.data as OrbitalNodeData).entityType === "agent"
              ? "#59c7a9"
              : "#6b7280"
          }
          nodeStrokeColor="#0a0e12"
          pannable
          zoomable
        />
      </ReactFlow>
    </div>
  );
}
