import { useEffect, useState } from "react";
import { scanMcpInventory, toggleMcpServer } from "../../lib/invoke";
import type { McpInventory, McpToggleResult } from "../../lib/types";
import { canToggleServer, existingSources, riskCounts } from "./mcpModel";

export function McpView() {
  const [inventory, setInventory] = useState<McpInventory | null>(null);
  const [status, setStatus] = useState("Preparing read-only MCP inventory.");
  const [scanning, setScanning] = useState(true);
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [lastToggle, setLastToggle] = useState<McpToggleResult | null>(null);

  async function scan(): Promise<void> {
    setScanning(true);
    setStatus("Scanning known MCP config locations...");
    try {
      const nextInventory = await scanMcpInventory();
      setInventory(nextInventory);
      setStatus(
        `Found ${nextInventory.servers.length} server definitions across ${
          existingSources(nextInventory.sources).length
        } detected config sources.`,
      );
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setStatus(`MCP inventory failed: ${detail}`);
    } finally {
      setScanning(false);
    }
  }

  async function toggleServer(serverId: string, enabled: boolean): Promise<void> {
    setTogglingId(serverId);
    setStatus(`${enabled ? "Enabling" : "Disabling"} MCP server...`);
    try {
      const result = await toggleMcpServer(serverId, enabled);
      setLastToggle(result);
      const nextInventory = await scanMcpInventory();
      setInventory(nextInventory);
      setStatus(
        `${result.serverName} ${enabled ? "enabled" : "disabled"}. Backup saved to ${result.backupPath}.`,
      );
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setStatus(`MCP toggle failed: ${detail}`);
    } finally {
      setTogglingId(null);
    }
  }

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      try {
        const nextInventory = await scanMcpInventory();
        if (!cancelled) {
          setInventory(nextInventory);
          setStatus(
            `Found ${nextInventory.servers.length} server definitions across ${
              existingSources(nextInventory.sources).length
            } detected config sources.`,
          );
        }
      } catch (error) {
        if (!cancelled) {
          const detail = error instanceof Error ? error.message : String(error);
          setStatus(`MCP inventory failed: ${detail}`);
        }
      } finally {
        if (!cancelled) {
          setScanning(false);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  const detectedSources = inventory ? existingSources(inventory.sources) : [];
  const counts = inventory
    ? riskCounts(inventory.servers)
    : { low: 0, medium: 0, high: 0 };

  return (
    <section className="workspace mcp-workspace">
      <header>
        <div>
          <p className="eyebrow">Phase 5 / Control Plane</p>
          <h2>MCP Servers</h2>
          <p>
            Inspect configured MCP transports, commands, environment key names,
            and risk indicators. JSON configs can be safely enabled or disabled
            with automatic backup/restore.
          </p>
        </div>
        <button
          className="refresh-button"
          disabled={scanning}
          onClick={() => void scan()}
          type="button"
        >
          {scanning ? "Scanning..." : "Refresh inventory"}
        </button>
      </header>

      <div className="mcp-status" role="status">
        <span className={scanning ? "pulse indicator" : "indicator"} />
        <span>{status}</span>
        <span className="risk-count low">{counts.low} low</span>
        <span className="risk-count medium">{counts.medium} medium</span>
        <span className="risk-count high">{counts.high} high</span>
      </div>

      {lastToggle ? (
        <p className="mcp-toggle-note">
          Last backup: <code>{lastToggle.backupPath}</code>
        </p>
      ) : null}

      <section className="mcp-source-strip" aria-label="MCP config sources">
        {detectedSources.map((source) => (
          <article key={source.id}>
            <div>
              <strong>{source.client}</strong>
              <span>{source.parsed ? "Parsed" : "Unavailable"}</span>
            </div>
            <p>{source.path}</p>
            <small>
              {source.error ?? `${source.serverCount} server definitions`}
            </small>
          </article>
        ))}
      </section>

      <section className="mcp-server-grid" aria-label="MCP server definitions">
        {inventory?.servers.length ? (
          inventory.servers.map((server) => (
            <article className="mcp-server-card" key={server.id}>
              <div className="mcp-card-heading">
                <div>
                  <p className="eyebrow">{server.client}</p>
                  <h3>{server.name}</h3>
                </div>
                <div className="mcp-card-badges">
                  <span className={`enabled-badge ${server.enabled ? "on" : "off"}`}>
                    {server.enabled ? "Enabled" : "Disabled"}
                  </span>
                  <span className={`risk-badge ${server.riskLevel}`}>
                    {server.riskLevel} risk
                  </span>
                </div>
              </div>

              <dl>
                <Detail label="Transport" value={server.transport} />
                <Detail
                  label="Command"
                  value={
                    server.command
                      ? `${server.command}${
                          server.commandAvailable === false
                            ? " (unavailable)"
                            : ""
                        }`
                      : "Remote transport"
                  }
                />
                {server.args.length ? (
                  <Detail label="Arguments" value={server.args.join(" ")} />
                ) : null}
                {server.cwd ? <Detail label="Working directory" value={server.cwd} /> : null}
                {server.url ? <Detail label="URL" value={server.url} /> : null}
                <Detail
                  label="Environment keys"
                  value={server.envKeys.length ? server.envKeys.join(", ") : "None"}
                />
                <Detail
                  label="Declared tools"
                  value={
                    server.declaredTools.length
                      ? server.declaredTools.join(", ")
                      : "Not declared"
                  }
                />
                <Detail label="Source" value={server.source} />
              </dl>

              <div className="risk-reasons">
                {server.riskReasons.map((reason) => (
                  <p key={reason}>{reason}</p>
                ))}
              </div>

              {canToggleServer(server) ? (
                <button
                  className="secondary-button"
                  disabled={togglingId === server.id}
                  onClick={() => void toggleServer(server.id, !server.enabled)}
                  type="button"
                >
                  {togglingId === server.id
                    ? "Updating..."
                    : server.enabled
                      ? "Disable server"
                      : "Enable server"}
                </button>
              ) : (
                <p className="mcp-toggle-note">Toggle unavailable for non-JSON sources.</p>
              )}
            </article>
          ))
        ) : (
          <div className="mcp-empty">
            <h3>No MCP server definitions found</h3>
            <p>
              Detected configuration sources will remain visible above,
              including parse or file-access errors.
            </p>
          </div>
        )}
      </section>
    </section>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}