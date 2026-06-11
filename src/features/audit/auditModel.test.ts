import { describe, expect, it } from "vitest";
import type { AuditEventRecord } from "../../lib/types";
import {
  auditStatusClass,
  formatAuditDuration,
  formatAuditTimestamp,
  hasMoreAuditEvents,
  mergeAuditEvents,
} from "./auditModel";

const event = (
  id: string,
  action: string,
  status: string,
  model: string,
): AuditEventRecord => ({
  id,
  action,
  status,
  model,
  conversationId: "thread:1",
  durationMs: 1250,
  createdAt: "2026-06-10T12:00:00Z",
});

describe("audit model helpers", () => {
  it("formats timestamps and durations", () => {
    expect(formatAuditDuration(450)).toBe("450ms");
    expect(formatAuditDuration(1500)).toBe("1.5s");
    expect(formatAuditTimestamp("invalid")).toBe("invalid");
  });

  it("maps audit status classes", () => {
    expect(auditStatusClass("completed")).toBe("audit-status completed");
    expect(auditStatusClass("failed")).toBe("audit-status failed");
  });

  it("tracks pagination and merges unique rows", () => {
    const first = [event("audit:1", "handoff.run", "completed", "grok-4.3")];
    const second = [
      event("audit:1", "handoff.run", "completed", "grok-4.3"),
      event("audit:2", "skill.execute", "failed", "hermes-local"),
    ];

    expect(hasMoreAuditEvents(1, 3)).toBe(true);
    expect(hasMoreAuditEvents(3, 3)).toBe(false);
    expect(mergeAuditEvents(first, second)).toHaveLength(2);
  });
});

describe("AuditView data contract", () => {
  it("renders with mock audit rows", () => {
    const rows = [
      event("audit:10", "handoff.run", "completed", "grok-4.3"),
      event("audit:11", "skill.execute", "failed", "hermes-local"),
    ];

    expect(rows.map((row) => row.action)).toEqual([
      "handoff.run",
      "skill.execute",
    ]);
    expect(formatAuditDuration(rows[0]!.durationMs)).toBe("1.3s");
    expect(auditStatusClass(rows[1]!.status)).toBe("audit-status failed");
  });
});