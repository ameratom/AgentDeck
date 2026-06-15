import { useEffect, useState } from "react";
import type {
  ChatgptReviewHealth,
  GrokMcpBridgeStatus,
  SecureTunnelStatus,
} from "../../../lib/types";
import {
  operationalChecks,
  reviewCheckClass,
  reviewReadyClass,
  reviewReadyLabel,
} from "../chatgptReviewModel";
import { McpDetail } from "./McpDetail";

type PopoverId = "bridge" | "tunnel" | "review" | null;

type McpStatusCardsProps = {
  bridgeStatus: GrokMcpBridgeStatus | null;
  tunnelStatus: SecureTunnelStatus | null;
  reviewHealth: ChatgptReviewHealth | null;
  syncingBridge: boolean;
  tunnelAction: "refresh" | "start" | "stop" | "open" | null;
  reviewRefreshing: boolean;
  onSyncBridge: () => void;
  onTunnelAction: (action: "refresh" | "start" | "stop" | "open") => void;
  onRefreshReview: () => void;
};

function basename(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

function bridgeStateLabel(bridgeStatus: GrokMcpBridgeStatus | null): string {
  if (!bridgeStatus) {
    return "Loading...";
  }
  if (bridgeStatus.hasKey) {
    return "Ready";
  }
  if (bridgeStatus.exists) {
    return "Missing key";
  }
  return "Not written";
}

function tunnelStateLabel(tunnelStatus: SecureTunnelStatus | null): string {
  if (!tunnelStatus) {
    return "Loading...";
  }
  if (tunnelStatus.ready) {
    return "Ready";
  }
  if (tunnelStatus.running) {
    return "Starting";
  }
  return "Stopped";
}

function tunnelStateClass(tunnelStatus: SecureTunnelStatus | null): string {
  if (!tunnelStatus) {
    return "tunnel-state";
  }
  if (tunnelStatus.ready) {
    return "tunnel-state ready";
  }
  if (tunnelStatus.running) {
    return "tunnel-state pending";
  }
  return "tunnel-state";
}

export function McpStatusCards({
  bridgeStatus,
  tunnelStatus,
  reviewHealth,
  syncingBridge,
  tunnelAction,
  reviewRefreshing,
  onSyncBridge,
  onTunnelAction,
  onRefreshReview,
}: McpStatusCardsProps) {
  const [openPopover, setOpenPopover] = useState<PopoverId>(null);

  useEffect(() => {
    if (!openPopover) {
      return;
    }
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.key === "Escape") {
        setOpenPopover(null);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [openPopover]);

  const reviewChecksPassed = reviewHealth
    ? operationalChecks(reviewHealth).filter((check) => check.passed).length
    : 0;
  const reviewChecksTotal = reviewHealth
    ? operationalChecks(reviewHealth).length
    : 0;

  return (
    <div className="mcp-status-row">
      <article className="mcp-scard">
        <div className="mcp-scard-top">
          <div>
            <p className="eyebrow">External connector</p>
            <h3>Grok MCP bridge</h3>
          </div>
          <span className="mcp-scard-pill">{bridgeStateLabel(bridgeStatus)}</span>
        </div>
        <dl className="mcp-scard-facts">
          <McpDetail
            label="Bridge file"
            value={
              bridgeStatus?.path ? basename(bridgeStatus.path) : "Not loaded"
            }
          />
          <McpDetail label="Status" value={bridgeStateLabel(bridgeStatus)} />
          <McpDetail
            label="Updated"
            value={bridgeStatus?.updatedAt ?? "—"}
          />
        </dl>
        <div className="mcp-scard-actions">
          <button
            className="secondary-button"
            disabled={syncingBridge}
            onClick={onSyncBridge}
            type="button"
          >
            {syncingBridge ? "Syncing..." : "Sync bridge"}
          </button>
          <button
            className="mcp-details-btn"
            onClick={() =>
              setOpenPopover(openPopover === "bridge" ? null : "bridge")
            }
            type="button"
          >
            Details ▾
          </button>
        </div>
        {openPopover === "bridge" ? (
          <div className="mcp-popover" role="dialog">
            <dl>
              <McpDetail
                label="Bridge file"
                value={bridgeStatus?.path ?? "Not loaded"}
              />
              <McpDetail label="Status" value={bridgeStateLabel(bridgeStatus)} />
              <McpDetail
                label="Updated"
                value={bridgeStatus?.updatedAt ?? "—"}
              />
            </dl>
            {bridgeStatus?.detail ? (
              <p className="mcp-toggle-note">{bridgeStatus.detail}</p>
            ) : null}
          </div>
        ) : null}
      </article>

      <article className="mcp-scard">
        <div className="mcp-scard-top">
          <div>
            <p className="eyebrow">Remote connector</p>
            <h3>Secure MCP Tunnel</h3>
          </div>
          <span className={tunnelStateClass(tunnelStatus)}>
            {tunnelStateLabel(tunnelStatus)}
          </span>
        </div>
        <dl className="mcp-scard-facts">
          <McpDetail
            label="Endpoint"
            value={tunnelStatus?.adminUrl ?? "Start tunnel first"}
          />
          <McpDetail
            label="Process"
            value={
              tunnelStatus?.pid
                ? `PID ${tunnelStatus.pid}`
                : "No AgentDeck-managed process"
            }
          />
          <McpDetail
            label="Health"
            value={tunnelStatus?.ready ? "Ready" : tunnelStatus?.running ? "Starting" : "Stopped"}
          />
        </dl>
        <div className="mcp-scard-actions">
          <button
            className="secondary-button"
            disabled={tunnelAction !== null}
            onClick={() => onTunnelAction("refresh")}
            type="button"
          >
            {tunnelAction === "refresh" ? "Refreshing..." : "Refresh status"}
          </button>
          <button
            className="secondary-button"
            disabled={
              tunnelAction !== null ||
              !tunnelStatus?.configured ||
              Boolean(tunnelStatus?.running || tunnelStatus?.ready)
            }
            onClick={() => onTunnelAction("start")}
            type="button"
          >
            {tunnelAction === "start" ? "Starting..." : "Start"}
          </button>
          <button
            className="mcp-details-btn"
            onClick={() =>
              setOpenPopover(openPopover === "tunnel" ? null : "tunnel")
            }
            type="button"
          >
            Details ▾
          </button>
        </div>
        {openPopover === "tunnel" ? (
          <div className="mcp-popover" role="dialog">
            <dl>
              <McpDetail
                label="Configuration"
                value={
                  tunnelStatus?.configured
                    ? tunnelStatus.configPath
                    : tunnelStatus?.configPath ?? "Loading..."
                }
              />
              <McpDetail
                label="Operator UI"
                value={tunnelStatus?.adminUrl ?? "Start tunnel first"}
              />
              <McpDetail
                label="Process"
                value={
                  tunnelStatus?.pid
                    ? `PID ${tunnelStatus.pid}`
                    : "No AgentDeck-managed process"
                }
              />
              <McpDetail label="Log" value={tunnelStatus?.logPath ?? "Loading..."} />
            </dl>
            {tunnelStatus?.detail ? (
              <p className="mcp-toggle-note">{tunnelStatus.detail}</p>
            ) : null}
            <div className="mcp-popover-actions">
              <button
                className="secondary-button"
                disabled={tunnelAction !== null || !tunnelStatus?.running}
                onClick={() => onTunnelAction("stop")}
                type="button"
              >
                {tunnelAction === "stop" ? "Stopping..." : "Stop tunnel"}
              </button>
              <button
                className="secondary-button"
                disabled={tunnelAction !== null || !tunnelStatus?.adminUrl}
                onClick={() => onTunnelAction("open")}
                type="button"
              >
                Open operator UI
              </button>
            </div>
          </div>
        ) : null}
      </article>

      <article className="mcp-scard">
        <div className="mcp-scard-top">
          <div>
            <p className="eyebrow">ChatGPT submission</p>
            <h3>Review readiness</h3>
          </div>
          <span
            className={
              reviewHealth
                ? reviewReadyClass(reviewHealth)
                : "chatgpt-review-state pending"
            }
          >
            {reviewHealth ? reviewReadyLabel(reviewHealth) : "Checking..."}
          </span>
        </div>
        <dl className="mcp-scard-facts">
          <McpDetail
            label="Platform"
            value={
              reviewHealth
                ? `${reviewHealth.platformStatus}`
                : "REVIEW"
            }
          />
          <McpDetail
            label="Checks"
            value={
              reviewHealth
                ? `${reviewChecksPassed}/${reviewChecksTotal} passed`
                : "Checking..."
            }
          />
          <McpDetail
            label="Tools"
            value={
              reviewHealth
                ? `${reviewHealth.submissionToolCount} read-only`
                : "Checking..."
            }
          />
        </dl>
        <div className="mcp-scard-actions">
          <button
            className="secondary-button"
            disabled={reviewRefreshing}
            onClick={onRefreshReview}
            type="button"
          >
            {reviewRefreshing ? "Checking..." : "Run review checks"}
          </button>
          <button
            className="mcp-details-btn"
            onClick={() =>
              setOpenPopover(openPopover === "review" ? null : "review")
            }
            type="button"
          >
            Details ▾
          </button>
        </div>
        {openPopover === "review" ? (
          <div className="mcp-popover mcp-popover--wide" role="dialog">
            <dl>
              <McpDetail
                label="Platform status"
                value={reviewHealth?.platformStatus ?? "REVIEW"}
              />
              <McpDetail
                label="Publish"
                value={
                  reviewHealth?.publishAllowed
                    ? "Allowed"
                    : reviewHealth?.publishBlockedReason ??
                      "Awaiting OpenAI approval"
                }
              />
              <McpDetail
                label="Submission tools"
                value={
                  reviewHealth
                    ? `${reviewHealth.submissionToolCount} read-only tools`
                    : "Checking local MCP profile..."
                }
              />
              <McpDetail
                label="Public MCP URL"
                value={
                  reviewHealth?.publicMcpUrl ??
                  "Set MCP_PUBLIC_RESOURCE_URL in tunnel env"
                }
              />
              <McpDetail
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
            <a
              className="chatgpt-review-link"
              href="https://platform.openai.com/apps-manage"
              rel="noreferrer"
              target="_blank"
            >
              Open Apps dashboard
            </a>
          </div>
        ) : null}
      </article>

      {openPopover ? (
        <button
          aria-label="Close details"
          className="mcp-popover-scrim"
          onClick={() => setOpenPopover(null)}
          type="button"
        />
      ) : null}
    </div>
  );
}