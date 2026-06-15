import { useEffect } from "react";
import type { McpServerDefinition } from "../../../lib/types";
import { canToggleServer } from "../mcpModel";
import { commandLabel } from "../mcpTableModel";
import { McpDetail } from "./McpDetail";

type McpServerDrawerProps = {
  server: McpServerDefinition | null;
  togglingId: string | null;
  onClose: () => void;
  onToggle: (serverId: string, enabled: boolean) => void;
};

export function McpServerDrawer({
  server,
  togglingId,
  onClose,
  onToggle,
}: McpServerDrawerProps) {
  useEffect(() => {
    if (!server) {
      return;
    }
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.key === "Escape") {
        onClose();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [server, onClose]);

  return (
    <>
      <button
        aria-label="Close server details"
        className={`mcp-drawer-scrim ${server ? "open" : ""}`}
        onClick={onClose}
        tabIndex={server ? 0 : -1}
        type="button"
      />
      <aside
        aria-hidden={!server}
        aria-labelledby="mcp-drawer-title"
        className={`mcp-drawer ${server ? "open" : ""}`}
        role="dialog"
      >
        {server ? (
          <>
            <div className="mcp-drawer-header">
              <div>
                <p className="eyebrow">{server.client}</p>
                <h3 id="mcp-drawer-title">{server.name}</h3>
              </div>
              <button
                aria-label="Close"
                className="mcp-drawer-close"
                onClick={onClose}
                type="button"
              >
                ✕
              </button>
            </div>

            <div className="mcp-drawer-badges">
              <span className={`enabled-badge ${server.enabled ? "on" : "off"}`}>
                {server.enabled ? "Enabled" : "Disabled"}
              </span>
              <span className={`risk-badge ${server.riskLevel}`}>
                {server.riskLevel} risk
              </span>
            </div>

            <dl className="mcp-drawer-details">
              <McpDetail label="Transport" value={server.transport} />
              <McpDetail label="Command / URL" value={commandLabel(server)} />
              {server.args.length ? (
                <McpDetail label="Arguments" value={server.args.join(" ")} />
              ) : null}
              {server.cwd ? (
                <McpDetail label="Working directory" value={server.cwd} />
              ) : null}
              <McpDetail
                label="Environment keys"
                value={server.envKeys.length ? server.envKeys.join(", ") : "None"}
              />
              <McpDetail
                label="Declared tools"
                value={
                  server.declaredTools.length
                    ? server.declaredTools.join(", ")
                    : "Not declared"
                }
              />
              <McpDetail label="Source" value={server.source} />
            </dl>

            {server.riskReasons.length ? (
              <div className="risk-reasons mcp-drawer-risks">
                {server.riskReasons.map((reason) => (
                  <p key={reason}>{reason}</p>
                ))}
              </div>
            ) : null}

            <div className="mcp-drawer-footer">
              {canToggleServer(server) ? (
                <button
                  className="secondary-button"
                  disabled={togglingId === server.id}
                  onClick={() => onToggle(server.id, !server.enabled)}
                  type="button"
                >
                  {togglingId === server.id
                    ? "Updating..."
                    : server.enabled
                      ? "Disable server"
                      : "Enable server"}
                </button>
              ) : (
                <p className="mcp-toggle-note">
                  Toggle unavailable for non-JSON sources.
                </p>
              )}
            </div>
          </>
        ) : null}
      </aside>
    </>
  );
}