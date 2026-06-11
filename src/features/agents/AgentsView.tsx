import { emit } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { loadAgentPermissions, setAgentPermission } from "../../lib/invoke";
import type { AgentPermissionMatrix, EnvironmentScan } from "../../lib/types";
import {
  agentConfigCount,
  agentPid,
  agentStatusClass,
  agentStatusLabel,
  agentVersion,
  filterAgents,
  formatAgentId,
  formatPermissionAction,
  permissionAllowed,
} from "./agentModel";

interface AgentsViewProps {
  scan: EnvironmentScan | null;
  onRefresh: () => void;
  busy: boolean;
}

export function AgentsView({ scan, onRefresh, busy }: AgentsViewProps) {
  const agents = filterAgents(scan?.entities ?? []);
  const [matrix, setMatrix] = useState<AgentPermissionMatrix | null>(null);
  const [permissionStatus, setPermissionStatus] = useState(
    "Loading permission matrix...",
  );
  const [updatingKey, setUpdatingKey] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      try {
        const nextMatrix = await loadAgentPermissions();
        if (!cancelled) {
          setMatrix(nextMatrix);
          setPermissionStatus("Permission matrix loaded.");
        }
      } catch (error) {
        if (!cancelled) {
          const detail = error instanceof Error ? error.message : String(error);
          setPermissionStatus(`Permission load failed: ${detail}`);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  async function openInGraph(entityId: string): Promise<void> {
    await emit("select-entity", { entityId });
  }

  async function togglePermission(
    agentId: string,
    action: string,
    allow: boolean,
  ): Promise<void> {
    const key = `${agentId}:${action}`;
    setUpdatingKey(key);
    setPermissionStatus(`Updating ${formatPermissionAction(action)}...`);
    try {
      const nextMatrix = await setAgentPermission(agentId, action, allow);
      setMatrix(nextMatrix);
      setPermissionStatus("Permission updated.");
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setPermissionStatus(`Permission update failed: ${detail}`);
    } finally {
      setUpdatingKey(null);
    }
  }

  return (
    <section className="workspace agents-workspace">
      <header>
        <div>
          <p className="eyebrow">Phase 5 / Control Plane</p>
          <h2>Agent Inventory</h2>
          <p>
            Live status for local agents discovered from tools, configs, and
            running processes. Configure per-agent permissions for config
            writes, handoffs, skills, and MCP tool calls.
          </p>
        </div>
        <button
          className="refresh-button"
          disabled={busy}
          onClick={onRefresh}
          type="button"
        >
          {busy ? "Scanning..." : "Refresh"}
        </button>
      </header>

      <div className="agent-page-status" role="status">
        <span className={busy ? "pulse indicator" : "indicator"} />
        <span>
          {scan
            ? `${agents.length} agents tracked • updated ${scan.scannedAt}`
            : "Waiting for the first environment scan..."}
        </span>
      </div>

      <section className="agent-grid" aria-label="Discovered agents">
        {agents.length === 0 ? (
          <p className="empty-state">No agents discovered yet.</p>
        ) : (
          agents.map((agent) => {
            const pid = agentPid(agent);
            return (
              <article className="agent-card" key={agent.id}>
                <div className="agent-card-heading">
                  <div>
                    <p className="eyebrow">{agent.source}</p>
                    <h3>{agent.name}</h3>
                  </div>
                  <span className={agentStatusClass(agent.status)}>
                    {agentStatusLabel(agent.status)}
                  </span>
                </div>

                <dl>
                  <div>
                    <dt>Version</dt>
                    <dd>{agentVersion(agent)}</dd>
                  </div>
                  <div>
                    <dt>PID</dt>
                    <dd>{pid ?? "—"}</dd>
                  </div>
                  <div>
                    <dt>Configs</dt>
                    <dd>{agentConfigCount(agent)}</dd>
                  </div>
                </dl>

                <button
                  className="secondary-button"
                  onClick={() => void openInGraph(agent.id)}
                  type="button"
                >
                  Open in Graph
                </button>
              </article>
            );
          })
        )}
      </section>

      <section className="permission-matrix-section" aria-label="Agent permissions">
        <div className="permission-matrix-header">
          <div>
            <h3>Permission Matrix</h3>
            <p>{permissionStatus}</p>
          </div>
        </div>

        {matrix ? (
          <div className="permission-matrix-wrap">
            <table className="permission-matrix">
              <thead>
                <tr>
                  <th scope="col">Agent</th>
                  {matrix.actions.map((action) => (
                    <th key={action} scope="col">
                      {formatPermissionAction(action)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {matrix.agents.map((agentId) => (
                  <tr key={agentId}>
                    <th scope="row">{formatAgentId(agentId)}</th>
                    {matrix.actions.map((action) => {
                      const allowed = permissionAllowed(matrix, agentId, action);
                      const key = `${agentId}:${action}`;
                      return (
                        <td key={key}>
                          <button
                            aria-pressed={allowed}
                            className={`permission-toggle ${allowed ? "allowed" : "denied"}`}
                            disabled={updatingKey === key}
                            onClick={() =>
                              void togglePermission(agentId, action, !allowed)
                            }
                            type="button"
                          >
                            {allowed ? "Allow" : "Deny"}
                          </button>
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="empty-state">Loading permissions...</p>
        )}
      </section>
    </section>
  );
}