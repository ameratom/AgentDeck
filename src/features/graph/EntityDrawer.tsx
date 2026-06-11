import type { DiscoveredEntity } from "../../lib/types";

interface EntityDrawerProps {
  entity: DiscoveredEntity | null;
  onClose: () => void;
}

export function EntityDrawer({ entity, onClose }: EntityDrawerProps) {
  if (!entity) {
    return (
      <aside className="entity-drawer empty">
        <p className="eyebrow">Entity details</p>
        <h3>Select a node</h3>
        <p>Choose an entity in the graph to inspect its normalized metadata.</p>
      </aside>
    );
  }

  return (
    <aside className="entity-drawer">
      <div className="drawer-heading">
        <div>
          <p className="eyebrow">{entity.entityType}</p>
          <h3>{entity.name}</h3>
        </div>
        <button aria-label="Close entity details" onClick={onClose} type="button">
          Close
        </button>
      </div>

      <dl>
        <div>
          <dt>Status</dt>
          <dd>{entity.status}</dd>
        </div>
        <div>
          <dt>ID</dt>
          <dd>{entity.id}</dd>
        </div>
        <div>
          <dt>Source</dt>
          <dd>{entity.source}</dd>
        </div>
        {Object.entries(entity.metadata).map(([key, value]) => (
          <div key={key}>
            <dt>{key}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
    </aside>
  );
}
