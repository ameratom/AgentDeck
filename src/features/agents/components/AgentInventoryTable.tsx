import type { KeyboardEvent } from "react";
import type { DiscoveredEntity } from "../../../lib/types";
import {
  agentConfigCount,
  agentPid,
  agentStatusLabel,
  agentVersion,
} from "../agentModel";

interface AgentInventoryTableProps {
  agents: DiscoveredEntity[];
  onRowClick: (agentId: string) => void;
}

function statusDotClass(status: string): string {
  switch (status) {
    case "running":
    case "available":
    case "configured":
    case "unavailable":
      return status;
    default:
      return "unavailable";
  }
}

function sourcePillClass(source: string): string {
  return source === "xai" ? "xai" : "";
}

function rowDimmed(status: string): boolean {
  return status === "configured" || status === "unavailable";
}

export function AgentInventoryTable({
  agents,
  onRowClick,
}: AgentInventoryTableProps) {
  function handleRowKeyDown(
    event: KeyboardEvent<HTMLDivElement>,
    agentId: string,
  ): void {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onRowClick(agentId);
    }
  }

  return (
    <section className="ag-panel ag-inventory" aria-label="Discovered agents">
      <div className="ag-panel-head">
        <div>
          <p className="ag-t-eyebrow">Agent discovery</p>
          <h3>Inventory</h3>
        </div>
        <p className="ag-panel-meta">
          From tools, configs
          <br />
          &amp; running processes
        </p>
      </div>

      <div className="ag-thead" aria-hidden="true">
        <span />
        <span>Agent</span>
        <span>Source</span>
        <span>Version</span>
        <span>PID</span>
        <span>Configs</span>
        <span />
      </div>

      <div className="ag-tbody">
        {agents.length === 0 ? (
          <p className="empty-state">No agents discovered yet.</p>
        ) : (
          agents.map((agent) => {
            const version = agentVersion(agent);
            const pid = agentPid(agent);
            return (
              <div
                key={agent.id}
                className={`ag-trow ${rowDimmed(agent.status) ? "off" : ""}`}
                onClick={() => onRowClick(agent.id)}
                onKeyDown={(event) => handleRowKeyDown(event, agent.id)}
                role="button"
                tabIndex={0}
              >
                <div className="ag-cell ag-c-status">
                  <span
                    className={`ag-sdot ${statusDotClass(agent.status)}`}
                    aria-hidden="true"
                  />
                </div>
                <div className="ag-cell ag-c-name">
                  <b title={agent.name}>{agent.name}</b>
                  <span className={`ag-stat ${statusDotClass(agent.status)}`}>
                    {agentStatusLabel(agent.status).toLowerCase()}
                  </span>
                </div>
                <div className="ag-cell">
                  <span className={`ag-srcpill ${sourcePillClass(agent.source)}`}>
                    {agent.source}
                  </span>
                </div>
                <div className="ag-cell ag-cell-mono" title={version}>
                  {version}
                </div>
                <div className="ag-cell ag-cell-num">{pid ?? "—"}</div>
                <div className="ag-cell ag-cell-num">
                  {agentConfigCount(agent)}
                </div>
                <div className="ag-cell ag-c-go" aria-hidden="true">
                  <svg
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    viewBox="0 0 24 24"
                  >
                    <path d="M5 12h14M13 6l6 6-6 6" />
                  </svg>
                </div>
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}