import { useMemo, useState } from "react";
import type { DiscoveredEntity } from "../../lib/types";

interface EntityListProps {
  entities: DiscoveredEntity[];
  selectedId: string | null;
  onSelect: (entity: DiscoveredEntity) => void;
}

const entityTypeLabels: Record<string, string> = {
  agent: "Agent",
  provider: "Provider",
  tool: "Tool",
  process: "Process",
  config: "Config",
};

export function EntityList({ entities, selectedId, onSelect }: EntityListProps) {
  const [search, setSearch] = useState("");
  const [filterType, setFilterType] = useState<string>("all");

  const filteredEntities = useMemo(() => {
    return entities
      .filter((entity) => {
        const matchesSearch =
          entity.name.toLowerCase().includes(search.toLowerCase()) ||
          entity.entityType.toLowerCase().includes(search.toLowerCase());

        const matchesType =
          filterType === "all" || entity.entityType === filterType;

        return matchesSearch && matchesType;
      })
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [entities, search, filterType]);

  const types = useMemo(() => {
    const uniqueTypes = new Set(entities.map((e) => e.entityType));
    return Array.from(uniqueTypes).sort();
  }, [entities]);

  return (
    <div className="entity-list">
      <div className="entity-list-header">
        <input
          className="entity-search"
          placeholder="Search entities..."
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />

        <select
          className="entity-filter"
          value={filterType}
          onChange={(e) => setFilterType(e.target.value)}
        >
          <option value="all">All Types</option>
          {types.map((type) => (
            <option key={type} value={type}>
              {entityTypeLabels[type] ?? type}
            </option>
          ))}
        </select>
      </div>

      <div className="entity-list-items">
        {filteredEntities.length === 0 && (
          <div className="entity-list-empty">No entities found.</div>
        )}

        {filteredEntities.map((entity) => {
          const isSelected = entity.id === selectedId;
          return (
            <button
              key={entity.id}
              className={`entity-list-item ${isSelected ? "selected" : ""}`}
              onClick={() => onSelect(entity)}
              type="button"
            >
              <div className="entity-item-main">
                <span className="entity-name">{entity.name}</span>
                <span className={`entity-badge ${entity.entityType}`}>
                  {entityTypeLabels[entity.entityType] ?? entity.entityType}
                </span>
              </div>
              <div className="entity-meta">
                <span className={`status-dot status-${entity.status}`} />
                <span className="entity-status">{entity.status}</span>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
