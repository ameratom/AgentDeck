import type { AgentPermissionMatrix } from "../../../lib/types";
import {
  formatAgentId,
  formatPermissionAction,
  permissionAllowed,
} from "../agentModel";

interface PermissionMatrixProps {
  matrix: AgentPermissionMatrix | null;
  permissionStatus: string;
  updatingKey: string | null;
  onToggle: (agentId: string, action: string, allow: boolean) => void;
}

const ACTION_LABELS: Record<string, { short: string; full: string }> = {
  "read-config": { short: "Read", full: "Read config" },
  "write-config": { short: "Write", full: "Write config" },
  "config-write": { short: "Config", full: "Config write" },
  "dispatch-handoff": { short: "Handoff", full: "Dispatch handoff" },
  "execute-skill": { short: "Skill", full: "Execute skill" },
  "call-mcp-tool": { short: "MCP", full: "Call MCP tool" },
};

const CALLER_COLUMN_PERCENT = 44;

const CALLER_NOTES: Record<string, string> = {
  "agent:agentdeck": "Owns all actions",
  "agent:claude-code": "Coding handoffs",
  "agent:codex": "Task handoffs",
  "agent:grok": "Research handoffs",
};

function actionLabel(action: string): { short: string; full: string } {
  return (
    ACTION_LABELS[action] ?? {
      short: formatPermissionAction(action),
      full: formatPermissionAction(action),
    }
  );
}

export function PermissionMatrix({
  matrix,
  permissionStatus,
  updatingKey,
  onToggle,
}: PermissionMatrixProps) {
  return (
    <section className="ag-panel ag-matrix" aria-label="Agent permissions">
      <div className="ag-panel-head">
        <div>
          <p className="ag-t-eyebrow">Access control</p>
          <h3>Permission Matrix</h3>
        </div>
        <p className="ag-panel-meta">{permissionStatus}</p>
      </div>

      <p className="ag-pm-note">
        Per-caller permissions for config writes, handoffs, skill execution, and
        MCP tool calls. Click a cell to allow or deny.
      </p>

      {matrix ? (
        <div className="ag-pm-wrap">
          <table className="ag-pm">
            <colgroup>
              <col style={{ width: `${CALLER_COLUMN_PERCENT}%` }} />
              {matrix.actions.map((action) => (
                <col
                  key={action}
                  style={{
                    width: `${(100 - CALLER_COLUMN_PERCENT) / matrix.actions.length}%`,
                  }}
                />
              ))}
            </colgroup>
            <thead>
              <tr>
                <th className="ag-pm-corner" scope="col">
                  Caller
                </th>
                {matrix.actions.map((action) => {
                  const label = actionLabel(action);
                  return (
                    <th
                      className="ag-pm-acth"
                      key={action}
                      scope="col"
                      title={label.full}
                    >
                      {label.short}
                    </th>
                  );
                })}
              </tr>
            </thead>
            <tbody>
              {matrix.agents.map((agentId) => (
                <tr key={agentId}>
                  <th className="ag-pm-rowh" scope="row">
                    <b>{formatAgentId(agentId)}</b>
                    {CALLER_NOTES[agentId] ? (
                      <small>{CALLER_NOTES[agentId]}</small>
                    ) : null}
                  </th>
                  {matrix.actions.map((action) => {
                    const allowed = permissionAllowed(matrix, agentId, action);
                    const key = `${agentId}:${action}`;
                    return (
                      <td className="ag-pm-cell" key={key}>
                        <button
                          aria-pressed={allowed}
                          className={`ag-ptog ${allowed ? "allowed" : "denied"}`}
                          disabled={updatingKey === key}
                          onClick={() => onToggle(agentId, action, !allowed)}
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
  );
}