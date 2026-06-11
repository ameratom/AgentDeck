import { useMemo, useState } from "react";
import type { DiscoveredEntity } from "../../lib/types";

interface EntitySelectorProps {
  entities: DiscoveredEntity[];
  selectedId: string | null;
  onSelect: (entity: DiscoveredEntity) => void;
  placeholder?: string;
}

export function EntitySelector({
  entities,
  selectedId,
  onSelect,
  placeholder = "Search entities...",
}: EntitySelectorProps) {
  const [search, setSearch] = useState("");
  const [isOpen, setIsOpen] = useState(false);

  const filtered = useMemo(() => {
    const term = search.toLowerCase();
    return entities
      .filter((e) =>
        e.name.toLowerCase().includes(term) ||
        e.entityType.toLowerCase().includes(term)
      )
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [entities, search]);

  const selectedEntity = entities.find((e) => e.id === selectedId);

  const handleSelect = (entity: DiscoveredEntity) => {
    onSelect(entity);
    setSearch("");
    setIsOpen(false);
  };

  return (
    <div className="entity-selector">
      <div
        className="entity-selector-trigger"
        onClick={() => setIsOpen(!isOpen)}
      >
        {selectedEntity ? (
          <span>{selectedEntity.name}</span>
        ) : (
          <span className="placeholder">{placeholder}</span>
        )}
        <span className="arrow">▼</span>
      </div>

      {isOpen && (
        <div className="entity-selector-dropdown">
          <input
            autoFocus
            className="entity-search-input"
            placeholder="Type to filter..."
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />

          <div className="entity-options">
            {filtered.length === 0 && (
              <div className="entity-option empty">No matches</div>
            )}

            {filtered.map((entity) => (
              <div
                key={entity.id}
                className={`entity-option ${entity.id === selectedId ? "selected" : ""}`}
                onClick={() => handleSelect(entity)}
              >
                <span className="name">{entity.name}</span>
                <span className="type">{entity.entityType}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
