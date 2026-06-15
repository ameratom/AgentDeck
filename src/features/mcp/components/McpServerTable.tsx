import { useMemo, useState } from "react";
import type { McpServerDefinition } from "../../../lib/types";
import { canToggleServer } from "../mcpModel";
import {
  commandLabel,
  filterServers,
  type ServerFilter,
} from "../mcpTableModel";

const FILTER_CHIPS: { id: ServerFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "enabled", label: "Enabled" },
  { id: "disabled", label: "Disabled" },
  { id: "low", label: "Low" },
  { id: "medium", label: "Medium" },
  { id: "high", label: "High" },
];

type McpServerTableProps = {
  servers: McpServerDefinition[];
  togglingId: string | null;
  onToggle: (serverId: string, enabled: boolean) => void;
  onRowClick: (serverId: string) => void;
};

export function McpServerTable({
  servers,
  togglingId,
  onToggle,
  onRowClick,
}: McpServerTableProps) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<ServerFilter>("all");

  const filteredServers = useMemo(
    () => filterServers(servers, query, filter),
    [servers, query, filter],
  );

  function handleRowKeyDown(
    event: React.KeyboardEvent<HTMLDivElement>,
    serverId: string,
  ): void {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onRowClick(serverId);
    }
  }

  return (
    <section className="mcp-table-panel" aria-label="MCP server definitions">
      <div className="mcp-table-toolbar">
        <input
          aria-label="Search MCP servers"
          className="mcp-table-search"
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search servers, sources, commands..."
          type="search"
          value={query}
        />
        <div className="mcp-filter-chips" role="group" aria-label="Server filters">
          {FILTER_CHIPS.map((chip) => (
            <button
              aria-pressed={filter === chip.id}
              className={
                filter === chip.id
                  ? "mcp-filter-chip active"
                  : "mcp-filter-chip"
              }
              key={chip.id}
              onClick={() => setFilter(chip.id)}
              type="button"
            >
              {chip.label}
            </button>
          ))}
        </div>
        <span className="mcp-table-count">
          {filteredServers.length} of {servers.length} shown
        </span>
      </div>

      <div className="mcp-thead" role="row">
        <div className="mcp-th" aria-hidden />
        <div className="mcp-th">Server</div>
        <div className="mcp-th">Source</div>
        <div className="mcp-th">Path</div>
        <div className="mcp-th">Transport</div>
        <div className="mcp-th">Command / URL</div>
        <div className="mcp-th">Env keys</div>
        <div className="mcp-th">Risk</div>
        <div className="mcp-th">On</div>
      </div>

      <div className="mcp-tbody">
        {filteredServers.length ? (
          filteredServers.map((server) => (
            <div
              className={`mcp-trow ${server.enabled ? "" : "is-disabled"}`}
              key={server.id}
              onClick={() => onRowClick(server.id)}
              onKeyDown={(event) => handleRowKeyDown(event, server.id)}
              role="button"
              tabIndex={0}
            >
              <div className="mcp-cell mcp-c-status">
                <span
                  className={`mcp-sdot ${server.enabled ? "on" : "off"}`}
                />
              </div>
              <div className="mcp-cell mcp-c-name" title={server.name}>
                {server.name}
              </div>
              <div className="mcp-cell mcp-c-client" title={server.client}>
                {server.client}
              </div>
              <div className="mcp-cell mcp-c-path" title={server.source}>
                {server.source}
              </div>
              <div className="mcp-cell">
                <span
                  className={`mcp-tline ${
                    server.transport === "http" ? "http" : ""
                  }`}
                >
                  {server.transport}
                </span>
              </div>
              <div
                className="mcp-cell mcp-mono"
                title={commandLabel(server)}
              >
                {commandLabel(server)}
              </div>
              <div className="mcp-cell mcp-mono">
                {server.envKeys.length ? (
                  server.envKeys.join(", ")
                ) : (
                  <span className="mcp-none">None</span>
                )}
              </div>
              <div className="mcp-cell">
                <span className={`risk-badge ${server.riskLevel}`}>
                  {server.riskLevel}
                </span>
              </div>
              <div className="mcp-cell mcp-c-action">
                {canToggleServer(server) ? (
                  <button
                    aria-label={
                      server.enabled ? "Disable server" : "Enable server"
                    }
                    aria-pressed={server.enabled}
                    className={`mcp-rtog ${server.enabled ? "on" : ""}`}
                    disabled={togglingId === server.id}
                    onClick={(event) => {
                      event.stopPropagation();
                      onToggle(server.id, !server.enabled);
                    }}
                    type="button"
                  />
                ) : (
                  <span
                    className="mcp-lock"
                    title="Toggle unavailable for non-JSON sources"
                  >
                    🔒
                  </span>
                )}
              </div>
            </div>
          ))
        ) : (
          <div className="mcp-empty mcp-empty--table">
            <h3>No matching MCP servers</h3>
            <p>Adjust search or filters to see server definitions.</p>
          </div>
        )}
      </div>
    </section>
  );
}