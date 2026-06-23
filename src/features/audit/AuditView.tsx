import { useEffect, useMemo, useState } from "react";
import { loadAuditEvents } from "../../lib/invoke";
import type { AuditEventRecord } from "../../lib/types";
import {
  auditActionLabel,
  auditStatusClass,
  auditStatusDotClass,
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
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);
  const [status, setStatus] = useState("Loading activity feed.");
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);

  const selectedEvent = useMemo(
    () => events.find((event) => event.id === selectedEventId) ?? null,
    [events, selectedEventId],
  );

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
        setSelectedEventId((current) => {
          if (current && page.events.some((event) => event.id === current)) {
            return current;
          }
          return page.events[0]?.id ?? null;
        });
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
    <section className="workspace audit-workspace audit-workspace--compact">
      <header className="au-compact-header">
        <div>
          <p className="eyebrow">Phase 3 / Activity</p>
          <h2>Audit Log</h2>
          <p className="au-compact-subtitle">
            Timestamped feed of handoff runs, skill executions, and scan-related
            actions. Handoff dispatch rows link to stored runs when available.
          </p>
        </div>
        <div className="au-compact-header-meta">
          <span className="phase-badge">Read-only</span>
          <div className="au-summary" role="status">
            <div className="au-scan-state">
              <span
                aria-hidden="true"
                className={loading ? "pulse indicator" : "indicator"}
              />
              <span>{status}</span>
            </div>
            <span className="au-pill">
              <b>{events.length}</b> shown
            </span>
            <span className="au-pill">
              <b>{total}</b> total
            </span>
          </div>
        </div>
      </header>

      <div className="au-body">
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

        <div className="au-master-detail">
          <section aria-label="Audit events" className="au-registry">
            <div className="section-heading">
              <div>
                <p className="eyebrow">Event feed</p>
                <h3>Recent activity</h3>
              </div>
              <small>{events.length} loaded</small>
            </div>

            <div className="au-registry-list" role="list">
              {events.length === 0 ? (
                <p className="empty-state">
                  {loading ? "Loading events..." : "No audit events recorded yet."}
                </p>
              ) : (
                events.map((event) => (
                  <button
                    aria-pressed={event.id === selectedEventId}
                    className={
                      event.id === selectedEventId
                        ? "au-registry-item selected"
                        : "au-registry-item"
                    }
                    key={event.id}
                    onClick={() => setSelectedEventId(event.id)}
                    role="listitem"
                    type="button"
                  >
                    <span
                      aria-hidden="true"
                      className={auditStatusDotClass(event.status)}
                    />
                    <span className="au-registry-copy">
                      <span className="au-registry-action">{event.action}</span>
                      <span className="au-registry-meta">
                        {formatAuditTimestamp(event.createdAt)}
                        {event.model ? ` · ${event.model}` : ""}
                      </span>
                    </span>
                    <span className={auditStatusClass(event.status)}>
                      {event.status}
                    </span>
                  </button>
                ))
              )}
            </div>

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

          <section aria-label="Audit event detail" className="au-detail-pane">
            {selectedEvent ? (
              <article className="au-detail-card">
                <div className="au-detail-heading">
                  <div>
                    <p className="eyebrow">Selected event</p>
                    <h3>{selectedEvent.action}</h3>
                    <p>{formatAuditTimestamp(selectedEvent.createdAt)}</p>
                  </div>
                  <span className={auditStatusClass(selectedEvent.status)}>
                    {selectedEvent.status}
                  </span>
                </div>

                <dl className="au-detail-grid">
                  <div>
                    <dt>Action</dt>
                    <dd>{selectedEvent.action}</dd>
                  </div>
                  <div>
                    <dt>Kind</dt>
                    <dd>{auditActionLabel(selectedEvent.action)}</dd>
                  </div>
                  <div>
                    <dt>Status</dt>
                    <dd>{selectedEvent.status}</dd>
                  </div>
                  <div>
                    <dt>Model</dt>
                    <dd>{selectedEvent.model || "—"}</dd>
                  </div>
                  <div>
                    <dt>Duration</dt>
                    <dd>{formatAuditDuration(selectedEvent.durationMs)}</dd>
                  </div>
                  <div>
                    <dt>Conversation</dt>
                    <dd>{selectedEvent.conversationId || "—"}</dd>
                  </div>
                  <div>
                    <dt>Event ID</dt>
                    <dd>{selectedEvent.id}</dd>
                  </div>
                  <div>
                    <dt>Handoff run</dt>
                    <dd>
                      {canOpenHandoffRun(selectedEvent) && selectedEvent.runId ? (
                        <button
                          className="audit-run-link"
                          onClick={() => onOpenHandoffRun?.(selectedEvent.runId!)}
                          type="button"
                        >
                          View run
                        </button>
                      ) : (
                        "—"
                      )}
                    </dd>
                  </div>
                </dl>
              </article>
            ) : (
              <p className="empty-state">
                {loading
                  ? "Loading event detail..."
                  : "Select an audit event to inspect details."}
              </p>
            )}
          </section>
        </div>
      </div>
    </section>
  );
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}