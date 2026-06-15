import { describe, expect, it } from "vitest";
import {
  routerAutoApplyKey,
  shouldAutoApplyRouter,
} from "./routerAutoApplyModel";

describe("routerAutoApplyModel", () => {
  const suggestion = {
    ruleId: "router-rule:test",
    ruleName: "Test rule",
    targetProviderId: "xai",
    targetModelId: "grok-4",
    reason: "Matched keyword review.",
  };

  it("builds stable auto-apply keys", () => {
    expect(routerAutoApplyKey("handoff:abc", suggestion)).toBe(
      "handoff:abc:router-rule:test:xai:grok-4",
    );
  });

  it("allows auto-apply only for fresh suggestions", () => {
    const key = routerAutoApplyKey("handoff:abc", suggestion);
    expect(
      shouldAutoApplyRouter(true, suggestion, "handoff:abc", null),
    ).toBe(true);
    expect(shouldAutoApplyRouter(true, suggestion, "handoff:abc", key)).toBe(
      false,
    );
    expect(shouldAutoApplyRouter(false, suggestion, "handoff:abc", null)).toBe(
      false,
    );
  });
});