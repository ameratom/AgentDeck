import type { HandoffRouteSuggestion } from "../../lib/types";
import { routerAutoApplyKey } from "./routerAutoApplyModel";

export function isRouterSuggestionAligned(
  suggestion: HandoffRouteSuggestion,
  selectedProviderId: string,
  selectedModelId: string,
): boolean {
  if (suggestion.targetProviderId !== selectedProviderId) {
    return false;
  }
  if (
    suggestion.targetModelId &&
    suggestion.targetModelId !== selectedModelId
  ) {
    return false;
  }
  return true;
}

export function shouldShowRouterSuggestion(
  suggestion: HandoffRouteSuggestion | null,
  selectedProviderId: string,
  selectedModelId: string,
  dismissedKey: string | null,
  requestKey: string,
): suggestion is HandoffRouteSuggestion {
  if (!suggestion || !requestKey) {
    return false;
  }

  const suggestionKey = routerAutoApplyKey(requestKey, suggestion);
  if (dismissedKey === suggestionKey) {
    return false;
  }

  return !isRouterSuggestionAligned(
    suggestion,
    selectedProviderId,
    selectedModelId,
  );
}

export function shouldAutoApplyRouterSuggestion(
  enabled: boolean,
  suggestion: HandoffRouteSuggestion | null,
  requestKey: string,
  lastAppliedKey: string | null,
  selectedProviderId: string,
  selectedModelId: string,
  userOverrodeProvider: boolean,
): suggestion is HandoffRouteSuggestion {
  if (!enabled || !suggestion || !requestKey || userOverrodeProvider) {
    return false;
  }

  if (
    isRouterSuggestionAligned(suggestion, selectedProviderId, selectedModelId)
  ) {
    return false;
  }

  const nextKey = routerAutoApplyKey(requestKey, suggestion);
  return nextKey !== lastAppliedKey;
}