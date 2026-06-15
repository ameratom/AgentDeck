import { useMemo, useState } from "react";
import type { PluginDefinition } from "../../../lib/types";
import {
  filterPlugins,
  type PluginFilter,
} from "../registryTableModel";

const FILTER_CHIPS: { id: PluginFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "enabled", label: "Enabled" },
  { id: "disabled", label: "Disabled" },
];

type PluginTableProps = {
  plugins: PluginDefinition[];
  busyId: string | null;
  onToggle: (pluginId: string, enabled: boolean) => void;
  onRowClick: (pluginId: string) => void;
};

export function PluginTable({
  plugins,
  busyId,
  onToggle,
  onRowClick,
}: PluginTableProps) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<PluginFilter>("all");

  const filteredPlugins = useMemo(
    () => filterPlugins(plugins, query, filter),
    [plugins, query, filter],
  );

  function handleRowKeyDown(
    event: React.KeyboardEvent<HTMLDivElement>,
    pluginId: string,
  ): void {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onRowClick(pluginId);
    }
  }

  return (
    <section
      aria-label="Plugin registry"
      className="reg-pane plugins"
    >
      <div className="reg-pane-head">
        <div>
          <p className="reg-pane-eyebrow">Integration modules</p>
          <h3>Plugin Registry</h3>
        </div>
        <span className="reg-pane-meta">Settings persist in AgentDeck only</span>
      </div>

      <div className="reg-pane-toolbar">
        <div className="reg-filters" role="group" aria-label="Plugin filters">
          {FILTER_CHIPS.map((chip) => (
            <button
              aria-pressed={filter === chip.id}
              className={
                filter === chip.id ? "reg-chip active" : "reg-chip"
              }
              key={chip.id}
              onClick={() => setFilter(chip.id)}
              type="button"
            >
              {chip.label}
            </button>
          ))}
        </div>
        <label className="reg-search">
          <span className="sr-only">Search plugins</span>
          <svg aria-hidden viewBox="0 0 24 24">
            <path
              d="M10.5 18a7.5 7.5 0 1 1 0-15 7.5 7.5 0 0 1 0 15Zm5.2-1.3 4.3 4.3"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            />
          </svg>
          <input
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search..."
            type="search"
            value={query}
          />
        </label>
      </div>

      <div className="reg-thead" role="row">
        <span aria-hidden />
        <span>Plugin</span>
        <span>Category</span>
        <span>Capabilities</span>
        <span>On</span>
      </div>

      <div className="reg-tbody">
        {filteredPlugins.length ? (
          filteredPlugins.map((plugin) => (
            <div
              className={`reg-trow ${plugin.enabled ? "" : "off"}`}
              key={plugin.id}
              onClick={() => onRowClick(plugin.id)}
              onKeyDown={(event) => handleRowKeyDown(event, plugin.id)}
              role="button"
              tabIndex={0}
            >
              <div className="reg-cell c-status">
                <span
                  className={`reg-sdot ${plugin.enabled ? "on" : "off"}`}
                />
              </div>
              <div className="reg-cell c-name" title={plugin.name}>
                {plugin.name}
              </div>
              <div className="reg-cell">
                <span className={`catpill ${plugin.category}`}>
                  {plugin.category}
                </span>
              </div>
              <div className="reg-cell">
                <div className="caps">
                  {plugin.capabilities.slice(0, 2).map((capability) => (
                    <span className="cap" key={capability}>
                      {capability}
                    </span>
                  ))}
                  {plugin.capabilities.length > 2 ? (
                    <span className="cap more">
                      +{plugin.capabilities.length - 2}
                    </span>
                  ) : null}
                </div>
              </div>
              <div className="reg-cell c-action">
                <button
                  aria-label={
                    plugin.enabled ? "Disable plugin" : "Enable plugin"
                  }
                  aria-pressed={plugin.enabled}
                  className={`reg-rtog ${plugin.enabled ? "on" : ""}`}
                  disabled={busyId !== null}
                  onClick={(event) => {
                    event.stopPropagation();
                    onToggle(plugin.id, !plugin.enabled);
                  }}
                  type="button"
                />
              </div>
            </div>
          ))
        ) : (
          <div className="reg-empty">
            <h3>No matching plugins</h3>
            <p>Adjust search or filters to see plugin definitions.</p>
          </div>
        )}
      </div>
    </section>
  );
}