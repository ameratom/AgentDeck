import { describe, expect, it } from "vitest";
import {
  createRouterRule,
  moveRouterRule,
  removeRouterRule,
  updateRouterRule,
} from "./routerModel";

describe("routerModel", () => {
  it("creates a rule with stable priority", () => {
    const rule = createRouterRule(2);
    expect(rule.priority).toBe(2);
    expect(rule.targetProviderId).toBe("lm-studio");
  });

  it("moves rules and reindexes priority", () => {
    const first = createRouterRule(0);
    const second = createRouterRule(1);
    second.id = "router-rule:second";
    const moved = moveRouterRule([first, second], second.id, "up");
    expect(moved[0]?.id).toBe(second.id);
    expect(moved[1]?.id).toBe(first.id);
    expect(moved[0]?.priority).toBe(0);
    expect(moved[1]?.priority).toBe(1);
  });

  it("updates and removes rules", () => {
    const rule = createRouterRule(0);
    const updated = updateRouterRule([rule], rule.id, { keyword: "review" });
    expect(updated[0]?.keyword).toBe("review");
    const removed = removeRouterRule(updated, rule.id);
    expect(removed).toHaveLength(0);
  });
});