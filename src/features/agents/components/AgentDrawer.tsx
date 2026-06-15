import { useEffect } from "react";
import type { DiscoveredEntity } from "../../../lib/types";
import {
  agentConfigCount,
  agentPid,
  agentStatusLabel,
  agentVersion,
} from "../agentModel";

interface AgentDrawerProps {
  agent: DiscoveredEntity | null;
  isCaller: boolean;
  onClose: () => void;
  onOpenInGraph: (agentId: string) => void;
}

const AGENT_ROLES: Record<string, string> = {
  "agent:claude-code": "Project-aware coding agent and handoff target.",
  "agent:codex": "OpenAI Codex CLI agent for tasks and handoffs.",
  "agent:grok": "xAI Grok agent reached via the Grok MCP bridge.",
  "agent:hermes": "Automation agent for monitoring and delegated tasks.",
  "agent:lm-studio": "Local OpenAI-compatible model host.",
  "agent:openclaw": "Local agent discovery and handoff placeholder.",
};

function statusBadgeClass(status: string): string {
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

export function AgentDrawer({
  agent,
  isCaller,
  onClose,
  onOpenInGraph,
}: AgentDrawerProps) {
  useEffect(() => {
    if (!agent) {
      return;
    }
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.key === "Escape") {
        onClose();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [agent, onClose]);

  const pid = agent ? agentPid(agent) : null;
  const role = agent ? AGENT_ROLES[agent.id] : undefined;

  return (
    <>
      <button
        aria-label="Close agent details"
        className={`ag-drawer-scrim ${agent ? "open" : ""}`}
        onClick={onClose}
        tabIndex={agent ? 0 : -1}
        type="button"
      />
      <aside
        aria-hidden={!agent}
        aria-labelledby="ag-drawer-title"
        className={`ag-drawer ${agent ? "open" : ""}`}
        role="dialog"
      >
        {agent ? (
          <>
            <div className="ag-drawer-head">
              <div className="ag-drawer-head-top">
                <div>
                  <p className="eyebrow ag-drawer-eyebrow">{agent.source}</p>
                  <h3 id="ag-drawer-title">{agent.name}</h3>
                </div>
                <button
                  aria-label="Close"
                  className="ag-drawer-close"
                  onClick={onClose}
                  type="button"
                >
                  ✕
                </button>
              </div>
              <div className="ag-drawer-badges">
                <span className={`ag-ebadge ${statusBadgeClass(agent.status)}`}>
                  <span className="dot" aria-hidden="true" />
                  {agentStatusLabel(agent.status)}
                </span>
              </div>
            </div>

            <div className="ag-drawer-body">
              <dl className="ag-ddl">
                {role ? (
                  <div>
                    <dt>Role</dt>
                    <dd className="ag-role">{role}</dd>
                  </div>
                ) : null}
                <div>
                  <dt>Version</dt>
                  <dd>{agentVersion(agent)}</dd>
                </div>
                <div>
                  <dt>Process</dt>
                  <dd>{pid === null ? "Not running" : `PID ${pid}`}</dd>
                </div>
                <div>
                  <dt>Config files</dt>
                  <dd>{agentConfigCount(agent)} detected</dd>
                </div>
                <div>
                  <dt>Source</dt>
                  <dd>{agent.source}</dd>
                </div>
                <div>
                  <dt>Agent ID</dt>
                  <dd>{agent.id}</dd>
                </div>
                <div>
                  <dt>Permissions</dt>
                  <dd className={isCaller ? "ag-perm-caller" : "ag-perm-discovered"}>
                    {isCaller
                      ? "Caller — see Permission Matrix for config / handoff / skill / MCP access."
                      : "Not a registered caller. Discovered for inventory and graph only."}
                  </dd>
                </div>
              </dl>
            </div>

            <div className="ag-drawer-foot">
              <button
                className="ag-btn"
                onClick={() => onOpenInGraph(agent.id)}
                type="button"
              >
                Open in Graph
              </button>
            </div>
          </>
        ) : null}
      </aside>
    </>
  );
}