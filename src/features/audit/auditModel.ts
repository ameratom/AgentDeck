import type { AuditEventRecord } from "../../lib/types";

export function isHandoffAuditAction(action: string): boolean {
  return action.startsWith("handoff.");
}

export function canOpenHandoffRun(event: AuditEventRecord): boolean {
  return isHandoffAuditAction(event.action) && Boolean(event.runId);
}

export function formatAuditTimestamp(value: string): string {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString();
}

export function formatAuditDuration(durationMs: number): string {
  if (durationMs < 1000) {
    return `${durationMs}ms`;
  }
  return `${(durationMs / 1000).toFixed(1)}s`;
}

export function auditStatusClass(status: string): string {
  switch (status) {
    case "completed":
      return "audit-status completed";
    case "failed":
      return "audit-status failed";
    case "running":
      return "audit-status running";
    default:
      return "audit-status";
  }
}

export function hasMoreAuditEvents(offset: number, total: number): boolean {
  return offset < total;
}

export function mergeAuditEvents(
  current: AuditEventRecord[],
  next: AuditEventRecord[],
): AuditEventRecord[] {
  const seen = new Set(current.map((event) => event.id));
  return [...current, ...next.filter((event) => !seen.has(event.id))];
}