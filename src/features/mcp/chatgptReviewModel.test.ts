import { describe, expect, it } from "vitest";
import type { ChatgptReviewHealth } from "../../lib/types";
import {
  operationalChecks,
  reviewCheckClass,
  reviewReadyClass,
  reviewReadyLabel,
} from "./chatgptReviewModel";

const sampleHealth = (): ChatgptReviewHealth => ({
  checkedAt: "2026-06-14T12:00:00Z",
  platformStatus: "REVIEW",
  publishAllowed: false,
  publishBlockedReason: "Awaiting approval",
  readyForReviewers: false,
  submissionToolCount: 10,
  publicMcpUrl: "https://mcp.example.com/mcp",
  checks: [
    {
      id: "platform-review",
      label: "Platform status",
      passed: true,
      detail: "In review",
    },
    {
      id: "publish-gate",
      label: "Publish gate",
      passed: false,
      detail: "Blocked",
    },
    {
      id: "tunnel-ready",
      label: "Tunnel",
      passed: false,
      detail: "Stopped",
    },
  ],
});

describe("chatgpt review model", () => {
  it("labels readiness from reviewer checks", () => {
    const health = sampleHealth();
    expect(reviewReadyLabel(health)).toBe("Action needed");
    expect(reviewReadyClass(health)).toBe("chatgpt-review-state pending");
    expect(operationalChecks(health)).toHaveLength(1);
    expect(reviewCheckClass(health.checks[2]!)).toBe("review-check failed");
  });

  it("marks ready state when reviewers can connect", () => {
    const health = { ...sampleHealth(), readyForReviewers: true };
    expect(reviewReadyLabel(health)).toBe("Ready for reviewers");
    expect(reviewReadyClass(health)).toBe("chatgpt-review-state ready");
  });
});