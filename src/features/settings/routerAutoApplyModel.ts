import type { HandoffRouteSuggestion } from "../../lib/types";

export function routerAutoApplyKey(
  requestKey: string,
  suggestion: HandoffRouteSuggestion,
): string {
  return `${requestKey}:${suggestion.ruleId}:${suggestion.targetProviderId}:${
    suggestion.targetModelId ?? ""
  }`;
}

export function shouldAutoApplyRouter(
  enabled: boolean,
  suggestion: HandoffRouteSuggestion | null,
  requestKey: string,
  lastAppliedKey: string | null,
): suggestion is HandoffRouteSuggestion {
  if (!enabled || !suggestion || !requestKey) {
    return false;
  }
  const nextKey = routerAutoApplyKey(requestKey, suggestion);
  return nextKey !== lastAppliedKey;
}