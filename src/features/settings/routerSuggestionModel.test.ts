import { describe, expect, it } from "vitest";
import {
  isRouterSuggestionAligned,
  shouldAutoApplyRouterSuggestion,
  shouldShowRouterSuggestion,
} from "./routerSuggestionModel";

describe("routerSuggestionModel", () => {
  const suggestion = {
    ruleId: "router-rule:code",
    ruleName: "Code implementation",
    targetProviderId: "codex",
    targetModelId: "codex-mini-latest",
    reason: 'Matched keyword "code".',
  };

  it("detects aligned provider and model selections", () => {
    expect(
      isRouterSuggestionAligned(suggestion, "codex", "codex-mini-latest"),
    ).toBe(true);
    expect(isRouterSuggestionAligned(suggestion, "xai", "codex-mini-latest")).toBe(
      false,
    );
    expect(isRouterSuggestionAligned(suggestion, "codex", "grok-4")).toBe(false);
  });

  it("hides suggestions that are already aligned", () => {
    expect(
      shouldShowRouterSuggestion(suggestion, "codex", "codex-mini-latest", null, "draft"),
    ).toBe(false);
    expect(
      shouldShowRouterSuggestion(suggestion, "xai", "grok-4", null, "draft"),
    ).toBe(true);
  });

  it("hides dismissed suggestions for the current draft", () => {
    expect(
      shouldShowRouterSuggestion(
        suggestion,
        "xai",
        "grok-4",
        "draft:router-rule:code:codex:codex-mini-latest",
        "draft",
      ),
    ).toBe(false);
  });

  it("skips auto-apply when aligned or overridden", () => {
    expect(
      shouldAutoApplyRouterSuggestion(
        true,
        suggestion,
        "draft",
        null,
        "codex",
        "codex-mini-latest",
        false,
      ),
    ).toBe(false);
    expect(
      shouldAutoApplyRouterSuggestion(
        true,
        suggestion,
        "draft",
        null,
        "xai",
        "grok-4",
        true,
      ),
    ).toBe(false);
    expect(
      shouldAutoApplyRouterSuggestion(
        true,
        suggestion,
        "draft",
        null,
        "xai",
        "grok-4",
        false,
      ),
    ).toBe(true);
  });
});