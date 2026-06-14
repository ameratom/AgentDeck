import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import {
  chatgptReviewHealth,
  grokMcpBridgeStatus,
  loadProjectConnectorSettings,
  openSecureTunnelUi,
  scanMcpInventory,
  secureTunnelStatus,
  saveProjectConnectorSettings,
  startSecureTunnel,
  stopSecureTunnel,
  syncGrokMcpBridge,
  toggleMcpServer,
} from "../../lib/invoke";
import type {
  ChatgptReviewHealth,
  GrokMcpBridgeStatus,
  McpInventory,
  McpToggleResult,
  ProjectConnectorSettings,
  SecureTunnelStatus,
} from "../../lib/types";
import {
  operationalChecks,
  reviewCheckClass,
  reviewReadyClass,
  reviewReadyLabel,
} from "./chatgptReviewModel";
import { canToggleServer, existingSources, riskCounts } from "./mcpModel";

export function McpView() {
  const [inventory, setInventory] = useState<McpInventory | null>(null);
  const [status, setStatus] = useState("Preparing read-only MCP inventory.");
  const [scanning, setScanning] = useState(true);
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [lastToggle, setLastToggle] = useState<McpToggleResult | null>(null);
  const [bridgeStatus, setBridgeStatus] = useState<GrokMcpBridgeStatus | null>(
    null,
  );
  const [syncingBridge, setSyncingBridge] = useState(false);
  const [tunnelStatus, setTunnelStatus] = useState<SecureTunnelStatus | null>(
    null,
  );
  const [tunnelAction, setTunnelAction] = useState<
    "refresh" | "start" | "stop" | "open" | null
  >(null);
  const [reviewHealth, setReviewHealth] = useState<ChatgptReviewHealth | null>(
    null,
  );
  const [reviewRefreshing, setReviewRefreshing] = useState(false);
  const [projectConnectors, setProjectConnectors] =
    useState<ProjectConnectorSettings | null>(null);
  const [savingProjectConnectors, setSavingProjectConnectors] = useState(false);

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

  async function syncBridge(): Promise<void> {
    setSyncingBridge(true);
    setStatus("Syncing Grok MCP bridge from encrypted xAI credentials...");
    try {
      const nextBridgeStatus = await syncGrokMcpBridge();
      setBridgeStatus(nextBridgeStatus);
      setStatus(nextBridgeStatus.detail);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setStatus(`Grok MCP bridge sync failed: ${detail}`);
    } finally {
      setSyncingBridge(false);
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

  async function refreshReviewHealth(): Promise<void> {
    setReviewRefreshing(true);
    try {
      const nextHealth = await chatgptReviewHealth();
      setReviewHealth(nextHealth);
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      setStatus(`ChatGPT review health check failed: ${detail}`);
    } finally {
      setReviewRefreshing(false);
    }
  }

  async function updateTunnel(
    action: "refresh" | "start" | "stop" | "open",
  ): Promise<void> {
    setTunnelAction(action);
    setStatus(
      action === "start"
        ? "Starting OpenAI Secure MCP Tunnel..."
        : action === "stop"
          ? "Stopping OpenAI Secure MCP Tunnel..."
          : action === "open"
            ? "Opening tunnel operator UI..."
            : "Refreshing tunnel status...",
    );
    try {
      const nextStatus =
        action === "start"
          ? await startSecureTunnel()
          : action === "stop"
            ? await stopSecureTunnel()
            : action === "open"
              ? await openSecureTunnelUi()
              : await secureTunnelStatus();
      setTunnelStatus(nextStatus);
      setStatus(nextStatus.detail);
      if (action === "start" || action === "refresh") {
        await refreshReviewHealth();
      }
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      const message = `Secure tunnel ${action} failed: ${detail}`;
      setStatus(message);
      setTunnelStatus((current) =>
        current
          ? { ...current, detail: message }
          : current,
      );
    } finally {
      setTunnelAction(null);
    }
  }

  async function saveProjectConnectors(): Promise<void> {
    if (!projectConnectors) {
      return;
    }
    setSavingProjectConnectors(true);
    setStatus(`Exporting connector profile for ${projectConnectors.projectName}...`);
    try {
      const nextSettings = await saveProjectConnectorSettings({
        filesystemEnabled: projectConnectors.filesystemEnabled,
        gitEnabled: projectConnectors.gitEnabled,
        claudeCodeServeEnabled: projectConnectors.claudeCodeServeEnabled,
        grokMcpEnabled: projectConnectors.grokMcpEnabled,
        xaiResearchMcpEnabled: projectConnectors.xaiResearchMcpEnabled,
      });
      const nextInventory = await scanMcpInventory();
      setProjectConnectors(nextSettings);
      setInventory(nextInventory);
      setStatus(
        `Project connector exports updated for ${nextSettings.projectName}. Existing client configs were not modified.`,
      );
    } catch (error) {
      setStatus(`Project connector export failed: ${formatError(error)}`);
    } finally {
      setSavingProjectConnectors(false);
    }
  }

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      try {
        const [nextInventory, nextBridgeStatus, nextTunnelStatus, nextProjectConnectors, nextReviewHealth] = await Promise.all([
          scanMcpInventory(),
          grokMcpBridgeStatus(),
          secureTunnelStatus(),
          loadProjectConnectorSettings().catch(() => null),
          chatgptReviewHealth().catch(() => null),
        ]);
        if (!cancelled) {
          setInventory(nextInventory);
          setBridgeStatus(nextBridgeStatus);
          setTunnelStatus(nextTunnelStatus);
          setProjectConnectors(nextProjectConnectors);
          setReviewHealth(nextReviewHealth);
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

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen("project-changed", () => {
      void Promise.all([
        scanMcpInventory(),
        loadProjectConnectorSettings().catch(() => null),
      ]).then(([nextInventory, nextSettings]) => {
        setInventory(nextInventory);
        setProjectConnectors(nextSettings);
      });
    }).then((dispose) => {
      unlisten = dispose;
    });
    return () => {
      unlisten?.();
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

      <section className="mcp-bridge-panel" aria-label="Grok MCP bridge">
        <div>
          <p className="eyebrow">External connector</p>
          <h3>Grok MCP bridge</h3>
          <p>
            Shell launchers cannot read AgentDeck&apos;s encrypted store. When you
            save an xAI key, AgentDeck mirrors it to a mode-0600 env file for{" "}
            <code>grok-mcp-launcher.sh</code>.
          </p>
        </div>
        <dl>
          <Detail
            label="Bridge file"
            value={bridgeStatus?.path ?? "Not loaded"}
          />
          <Detail
            label="Status"
            value={
              bridgeStatus
                ? bridgeStatus.hasKey
                  ? "Ready for grok-mcp"
                  : bridgeStatus.exists
                    ? "Present but missing key"
                    : "Not written"
                : "Loading..."
            }
          />
          {bridgeStatus?.updatedAt ? (
            <Detail label="Updated" value={bridgeStatus.updatedAt} />
          ) : null}
        </dl>
        <p className="mcp-toggle-note">{bridgeStatus?.detail}</p>
        <button
          className="secondary-button"
          disabled={syncingBridge}
          onClick={() => void syncBridge()}
          type="button"
        >
          {syncingBridge ? "Syncing..." : "Sync Grok MCP bridge"}
        </button>
      </section>

      {projectConnectors ? (
        <section
          className="mcp-project-connectors"
          aria-label="Active project connector profile"
        >
          <div className="mcp-project-connectors-heading">
            <div>
              <p className="eyebrow">Active project</p>
              <h3>{projectConnectors.projectName} connector profile</h3>
              <p>
                Generate project-bound Claude JSON and Codex TOML snippets.
                AgentDeck does not modify either client&apos;s configuration.
              </p>
            </div>
            <span className="project-state active">Export only</span>
          </div>
          <p className="workspace-context">{projectConnectors.projectPath}</p>
          <div className="mcp-project-connector-options">
            <label>
              <input
                checked={projectConnectors.filesystemEnabled}
                disabled={savingProjectConnectors}
                onChange={(event) =>
                  setProjectConnectors((current) =>
                    current
                      ? { ...current, filesystemEnabled: event.target.checked }
                      : current,
                  )
                }
                type="checkbox"
              />
              <span>
                <strong>Filesystem MCP</strong>
                <small>Read access constrained to this project root.</small>
              </span>
            </label>
            <label>
              <input
                checked={projectConnectors.gitEnabled}
                disabled={savingProjectConnectors}
                onChange={(event) =>
                  setProjectConnectors((current) =>
                    current
                      ? { ...current, gitEnabled: event.target.checked }
                      : current,
                  )
                }
                type="checkbox"
              />
              <span>
                <strong>Git MCP</strong>
                <small>Read-oriented status, log, branch, and diff tools.</small>
              </span>
            </label>
            <label>
              <input
                checked={projectConnectors.claudeCodeServeEnabled}
                disabled={savingProjectConnectors}
                onChange={(event) =>
                  setProjectConnectors((current) =>
                    current
                      ? {
                          ...current,
                          claudeCodeServeEnabled: event.target.checked,
                        }
                      : current,
                  )
                }
                type="checkbox"
              />
              <span>
                <strong>Claude Code MCP serve</strong>
                <small>
                  Export <code>claude mcp serve</code> for Codex and other MCP
                  clients.
                </small>
              </span>
            </label>
            <label>
              <input
                checked={projectConnectors.grokMcpEnabled}
                disabled={savingProjectConnectors}
                onChange={(event) =>
                  setProjectConnectors((current) =>
                    current
                      ? { ...current, grokMcpEnabled: event.target.checked }
                      : current,
                  )
                }
                type="checkbox"
              />
              <span>
                <strong>Grok MCP</strong>
                <small>Export grok-mcp via the AgentDeck bridge env file.</small>
              </span>
            </label>
            <label>
              <input
                checked={projectConnectors.xaiResearchMcpEnabled}
                disabled={savingProjectConnectors}
                onChange={(event) =>
                  setProjectConnectors((current) =>
                    current
                      ? {
                          ...current,
                          xaiResearchMcpEnabled: event.target.checked,
                        }
                      : current,
                  )
                }
                type="checkbox"
              />
              <span>
                <strong>xAI Research MCP</strong>
                <small>Read-only web research tools for local MCP clients.</small>
              </span>
            </label>
          </div>
          <dl>
            <Detail label="Claude export" value={projectConnectors.claudeExportPath} />
            <Detail label="Codex export" value={projectConnectors.codexExportPath} />
            {projectConnectors.claudeCodeServeEnabled ? (
              <Detail
                label="Claude Code serve"
                value={projectConnectors.claudeCodeServeExportPath}
              />
            ) : null}
          </dl>
          <button
            disabled={savingProjectConnectors}
            onClick={() => void saveProjectConnectors()}
            type="button"
          >
            {savingProjectConnectors ? "Exporting..." : "Save and export profile"}
          </button>
        </section>
      ) : null}

      <section className="chatgpt-review-panel" aria-label="ChatGPT app review readiness">
        <div className="mcp-tunnel-heading">
          <div>
            <p className="eyebrow">ChatGPT submission</p>
            <h3>Review readiness</h3>
            <p>
              While OpenAI reviews version 1.0.0, keep AgentDeck and the Secure
              MCP Tunnel running. Publishing stays blocked until status becomes
              Approved.
            </p>
          </div>
          <span
            className={
              reviewHealth ? reviewReadyClass(reviewHealth) : "chatgpt-review-state pending"
            }
          >
            {reviewHealth ? reviewReadyLabel(reviewHealth) : "Checking..."}
          </span>
        </div>
        <dl>
          <Detail
            label="Platform status"
            value={reviewHealth?.platformStatus ?? "REVIEW"}
          />
          <Detail
            label="Publish"
            value={
              reviewHealth?.publishAllowed
                ? "Allowed"
                : reviewHealth?.publishBlockedReason ?? "Awaiting OpenAI approval"
            }
          />
          <Detail
            label="Submission tools"
            value={
              reviewHealth
                ? `${reviewHealth.submissionToolCount} read-only tools`
                : "Checking local MCP profile..."
            }
          />
          <Detail
            label="Public MCP URL"
            value={reviewHealth?.publicMcpUrl ?? "Set MCP_PUBLIC_RESOURCE_URL in tunnel env"}
          />
          <Detail
            label="Last checked"
            value={reviewHealth?.checkedAt ?? "Not checked yet"}
          />
        </dl>
        {reviewHealth ? (
          <ul className="chatgpt-review-checks">
            {operationalChecks(reviewHealth).map((check) => (
              <li className={reviewCheckClass(check)} key={check.id}>
                <span>{check.label}</span>
                <small>{check.detail}</small>
              </li>
            ))}
          </ul>
        ) : null}
        <div className="mcp-tunnel-actions">
          <button
            className="secondary-button"
            disabled={reviewRefreshing}
            onClick={() => void refreshReviewHealth()}
            type="button"
          >
            {reviewRefreshing ? "Checking..." : "Run review checks"}
          </button>
          <a
            className="chatgpt-review-link"
            href="https://platform.openai.com/apps-manage"
            rel="noreferrer"
            target="_blank"
          >
            Open Apps dashboard
          </a>
        </div>
      </section>

      <section className="mcp-tunnel-panel" aria-label="OpenAI Secure MCP Tunnel">
        <div className="mcp-tunnel-heading">
          <div>
            <p className="eyebrow">Remote connector</p>
            <h3>OpenAI Secure MCP Tunnel</h3>
            <p>
              Start a scoped outbound connection from ChatGPT to AgentDeck&apos;s
              loopback MCP server. AgentDeck owns the PID, health URL, and log
              for processes started here.
            </p>
          </div>
          <span
            className={`tunnel-state ${
              tunnelStatus?.ready
                ? "ready"
                : tunnelStatus?.running
                  ? "pending"
                  : "stopped"
            }`}
          >
            {tunnelStatus?.ready
              ? "Ready"
              : tunnelStatus?.running
                ? "Starting"
                : "Stopped"}
          </span>
        </div>
        <dl>
          <Detail
            label="Configuration"
            value={
              tunnelStatus?.configured
                ? tunnelStatus.configPath
                : tunnelStatus?.configPath ?? "Loading..."
            }
          />
          <Detail
            label="Process"
            value={
              tunnelStatus?.pid
                ? `PID ${tunnelStatus.pid}`
                : "No AgentDeck-managed process"
            }
          />
          <Detail label="Operator UI" value={tunnelStatus?.adminUrl ?? "Start tunnel first"} />
          <Detail label="Log" value={tunnelStatus?.logPath ?? "Loading..."} />
        </dl>
        <p className="mcp-toggle-note">{tunnelStatus?.detail}</p>
        <div className="mcp-tunnel-actions">
          <button
            disabled={
              tunnelAction !== null ||
              !tunnelStatus?.configured ||
              tunnelStatus.running ||
              tunnelStatus.ready
            }
            onClick={() => void updateTunnel("start")}
            type="button"
          >
            {tunnelAction === "start" ? "Starting..." : "Start tunnel"}
          </button>
          <button
            className="secondary-button"
            disabled={tunnelAction !== null || !tunnelStatus?.running}
            onClick={() => void updateTunnel("stop")}
            type="button"
          >
            {tunnelAction === "stop" ? "Stopping..." : "Stop tunnel"}
          </button>
          <button
            className="secondary-button"
            disabled={tunnelAction !== null || !tunnelStatus?.adminUrl}
            onClick={() => void updateTunnel("open")}
            type="button"
          >
            Open operator UI
          </button>
          <button
            className="secondary-button"
            disabled={tunnelAction !== null}
            onClick={() => void updateTunnel("refresh")}
            type="button"
          >
            Refresh status
          </button>
        </div>
      </section>

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

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
