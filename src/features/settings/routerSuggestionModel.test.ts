import { describe, expect, it } from "vitest";
import { routerAutoApplyKey } from "./routerAutoApplyModel";
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

  const suggestionWithoutModel = {
    ...suggestion,
    targetModelId: null,
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

  it("treats unset target model as aligned when provider matches", () => {
    expect(
      isRouterSuggestionAligned(suggestionWithoutModel, "codex", "any-model"),
    ).toBe(true);
  });

  it("hides suggestions that are already aligned", () => {
    expect(
      shouldShowRouterSuggestion(suggestion, "codex", "codex-mini-latest", null, "draft"),
    ).toBe(false);
    expect(
      shouldShowRouterSuggestion(suggestion, "xai", "grok-4", null, "draft"),
    ).toBe(true);
  });

  it("hides suggestions when input is empty or missing", () => {
    expect(
      shouldShowRouterSuggestion(suggestion, "xai", "grok-4", null, ""),
    ).toBe(false);
    expect(
      shouldShowRouterSuggestion(null, "xai", "grok-4", null, "draft"),
    ).toBe(false);
  });

  it("hides dismissed suggestions for the current draft", () => {
    const dismissedKey = routerAutoApplyKey("draft", suggestion);
    expect(
      shouldShowRouterSuggestion(suggestion, "xai", "grok-4", dismissedKey, "draft"),
    ).toBe(false);
  });

  it("shows dismissed suggestions again after the draft changes", () => {
    const dismissedKey = routerAutoApplyKey("draft-a", suggestion);
    expect(
      shouldShowRouterSuggestion(
        suggestion,
        "xai",
        "grok-4",
        dismissedKey,
        "draft-b",
      ),
    ).toBe(true);
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

  it("skips auto-apply when disabled, already applied, or missing input", () => {
    const appliedKey = routerAutoApplyKey("draft", suggestion);
    expect(
      shouldAutoApplyRouterSuggestion(
        false,
        suggestion,
        "draft",
        null,
        "xai",
        "grok-4",
        false,
      ),
    ).toBe(false);
    expect(
      shouldAutoApplyRouterSuggestion(
        true,
        suggestion,
        "draft",
        appliedKey,
        "xai",
        "grok-4",
        false,
      ),
    ).toBe(false);
    expect(
      shouldAutoApplyRouterSuggestion(
        true,
        null,
        "draft",
        null,
        "xai",
        "grok-4",
        false,
      ),
    ).toBe(false);
    expect(
      shouldAutoApplyRouterSuggestion(
        true,
        suggestion,
        "",
        null,
        "xai",
        "grok-4",
        false,
      ),
    ).toBe(false);
  });
});