import { useEffect, useState } from "react";
import { loadAuditEvents } from "../../lib/invoke";
import type { AuditEventRecord } from "../../lib/types";
import {
  auditStatusClass,
  canOpenHandoffRun,
  formatAuditDuration,
  formatAuditTimestamp,
  hasMoreAuditEvents,
  mergeAuditEvents,
} from "./auditModel";

const PAGE_SIZE = 25;

interface AuditViewProps {
  onOpenHandoffRun?: (runId: string) => void;
}

export function AuditView({ onOpenHandoffRun }: AuditViewProps) {
  const [events, setEvents] = useState<AuditEventRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [filter, setFilter] = useState("");
  const [appliedFilter, setAppliedFilter] = useState("");
  const [status, setStatus] = useState("Loading activity feed.");
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function load(): Promise<void> {
      setLoading(true);
      setStatus("Loading activity feed...");
      try {
        const page = await loadAuditEvents(PAGE_SIZE, 0, appliedFilter || undefined);
        if (cancelled) {
          return;
        }
        setEvents(page.events);
        setTotal(page.total);
        setOffset(page.events.length);
        setStatus(`Loaded ${page.events.length} of ${page.total} events.`);
      } catch (error) {
        if (!cancelled) {
          setStatus(`Activity load failed: ${formatError(error)}`);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, [appliedFilter]);

  async function loadMore(): Promise<void> {
    setLoadingMore(true);
    try {
      const page = await loadAuditEvents(
        PAGE_SIZE,
        offset,
        appliedFilter || undefined,
      );
      setEvents((current) => mergeAuditEvents(current, page.events));
      setOffset((current) => current + page.events.length);
      setTotal(page.total);
      setStatus(`Loaded ${offset + page.events.length} of ${page.total} events.`);
    } catch (error) {
      setStatus(`Activity load failed: ${formatError(error)}`);
    } finally {
      setLoadingMore(false);
    }
  }

  function applyFilter(): void {
    setAppliedFilter(filter.trim());
  }

  return (
    <section className="workspace audit-workspace">
      <header>
        <div>
          <p className="eyebrow">Phase 3 / Activity</p>
          <h2>Audit Log</h2>
          <p>
            Timestamped feed of handoff runs, skill executions, and scan-related
            actions. Handoff dispatch rows link to stored runs when available.
          </p>
        </div>
        <span className="phase-badge">Read-only</span>
      </header>

      <div className="audit-controls">
        <input
          aria-label="Filter audit events"
          onChange={(event) => setFilter(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              applyFilter();
            }
          }}
          placeholder="Filter by action or model"
          type="search"
          value={filter}
        />
        <button onClick={applyFilter} type="button">
          Apply filter
        </button>
      </div>

      <div className="audit-page-status" role="status">
        <span className={loading ? "pulse indicator" : "indicator"} />
        <span>{status}</span>
      </div>

      <section className="audit-table" aria-label="Audit events">
        <div className="audit-table-header" role="row">
          <span>Timestamp</span>
          <span>Action</span>
          <span>Status</span>
          <span>Model</span>
          <span>Duration</span>
          <span>Handoff</span>
        </div>

        {events.length === 0 ? (
          <p className="empty-state">
            {loading ? "Loading events..." : "No audit events recorded yet."}
          </p>
        ) : (
          events.map((event) => (
            <div className="audit-table-row" key={event.id} role="row">
              <span>{formatAuditTimestamp(event.createdAt)}</span>
              <span>{event.action}</span>
              <span className={auditStatusClass(event.status)}>
                {event.status}
              </span>
              <span>{event.model || "—"}</span>
              <span>{formatAuditDuration(event.durationMs)}</span>
              <span>
                {canOpenHandoffRun(event) && event.runId ? (
                  <button
                    className="audit-run-link"
                    onClick={() => onOpenHandoffRun?.(event.runId!)}
                    type="button"
                  >
                    View run
                  </button>
                ) : (
                  "—"
                )}
              </span>
            </div>
          ))
        )}
      </section>

      {hasMoreAuditEvents(offset, total) && events.length > 0 ? (
        <button
          className="secondary-button load-more-button"
          disabled={loadingMore}
          onClick={() => void loadMore()}
          type="button"
        >
          {loadingMore ? "Loading..." : "Load more"}
        </button>
      ) : null}
    </section>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}