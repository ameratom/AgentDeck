import { emit } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import { loadAgentPermissions, setAgentPermission } from "../../lib/invoke";
import type { AgentPermissionMatrix, EnvironmentScan } from "../../lib/types";
import { AgentDrawer } from "./components/AgentDrawer";
import { AgentInventoryTable } from "./components/AgentInventoryTable";
import { PermissionMatrix } from "./components/PermissionMatrix";
import {
  filterAgents,
  formatPermissionAction,
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
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);

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

  const selectedAgent = useMemo(
    () => agents.find((agent) => agent.id === selectedAgentId) ?? null,
    [agents, selectedAgentId],
  );

  const runningCount = agents.filter((agent) => agent.status === "running").length;
  const isCaller =
    selectedAgent !== null && matrix !== null
      ? matrix.agents.includes(selectedAgent.id)
      : false;

  async function openInGraph(entityId: string): Promise<void> {
    await emit("select-entity", { entityId });
    setSelectedAgentId(null);
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
    <section className="workspace agents-workspace agents-workspace--compact">
      <header className="ag-compact-header">
        <div>
          <p className="eyebrow">Phase 5 / Control Plane</p>
          <h2>Agent Inventory</h2>
          <p className="ag-compact-subtitle">
            Live status for local agents discovered from tools, configs, and
            running processes. Configure per-agent permissions for config
            writes, handoffs, skills, and MCP tool calls.
          </p>
        </div>
        <div className="ag-compact-header-meta">
          <button
            className="refresh-button"
            disabled={busy}
            onClick={onRefresh}
            type="button"
          >
            {busy ? "Scanning..." : "Refresh"}
          </button>
          <div className="ag-summary">
            <div className="ag-scan-state" role="status">
              <span
                className={busy ? "pulse indicator" : "indicator"}
                aria-hidden="true"
              />
              <span>
                {scan
                  ? `updated ${scan.scannedAt}`
                  : "Waiting for the first environment scan..."}
              </span>
            </div>
            {scan ? (
              <>
                <span className="ag-pill">
                  <b>{agents.length}</b> tracked
                </span>
                <span className="ag-pill ag-pill--on">
                  <b>{runningCount}</b> running
                </span>
              </>
            ) : null}
          </div>
        </div>
      </header>

      <div className="ag-body">
        <AgentInventoryTable
          agents={agents}
          onRowClick={setSelectedAgentId}
        />
        <PermissionMatrix
          matrix={matrix}
          onToggle={(agentId, action, allow) => {
            void togglePermission(agentId, action, allow);
          }}
          permissionStatus={permissionStatus}
          updatingKey={updatingKey}
        />
      </div>

      <AgentDrawer
        agent={selectedAgent}
        isCaller={isCaller}
        onClose={() => setSelectedAgentId(null)}
        onOpenInGraph={(agentId) => {
          void openInGraph(agentId);
        }}
      />
    </section>
  );
}