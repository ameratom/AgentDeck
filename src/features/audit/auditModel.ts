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

export function auditStatusDotClass(status: string): string {
  switch (status) {
    case "completed":
      return "compact-status-dot completed";
    case "failed":
      return "compact-status-dot failed";
    case "running":
      return "compact-status-dot running";
    default:
      return "compact-status-dot pending";
  }
}

export function auditActionLabel(action: string): string {
  const parts = action.split(".");
  return parts[parts.length - 1] ?? action;
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