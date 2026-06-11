import { describe, expect, it } from "vitest";
import type { ToolStatus } from "../lib/types";

describe("Phase 0 types", () => {
  it("represents unavailable tools without throwing", () => {
    const tool: ToolStatus = {
      name: "openclaw",
      available: false,
      version: null,
      path: null,
      error: "unavailable",
    };

    expect(tool.available).toBe(false);
    expect(tool.error).toBe("unavailable");
  });
});
