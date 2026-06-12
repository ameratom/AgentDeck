import type { RouterRule } from "../../lib/types";

export const ROUTER_SOURCE_OPTIONS = [
  { id: "", label: "Any source agent" },
  { id: "agent:agentdeck", label: "AgentDeck" },
  { id: "agent:claude-code", label: "Claude Code" },
  { id: "agent:codex", label: "Codex" },
  { id: "agent:grok", label: "Grok" },
];

export const ROUTER_PROVIDER_OPTIONS = [
  { id: "lm-studio", label: "LM Studio" },
  { id: "xai", label: "xAI / Grok" },
  { id: "anthropic", label: "Anthropic" },
  { id: "openai-compatible", label: "OpenAI-compatible" },
  { id: "codex", label: "Codex" },
  { id: "claude-code", label: "Claude Code" },
];

export function createRouterRule(priority: number): RouterRule {
  const stamp = Date.now().toString(36);
  return {
    id: `router-rule:${stamp}`,
    priority,
    name: "New routing rule",
    enabled: true,
    sourceAgentId: null,
    keyword: null,
    targetProviderId: "lm-studio",
    targetModelId: null,
    updatedAt: new Date().toISOString(),
  };
}

export function moveRouterRule(
  rules: RouterRule[],
  ruleId: string,
  direction: "up" | "down",
): RouterRule[] {
  const index = rules.findIndex((rule) => rule.id === ruleId);
  if (index < 0) {
    return rules;
  }
  const targetIndex = direction === "up" ? index - 1 : index + 1;
  if (targetIndex < 0 || targetIndex >= rules.length) {
    return rules;
  }
  const next = [...rules];
  const [rule] = next.splice(index, 1);
  next.splice(targetIndex, 0, rule);
  return next.map((entry, entryIndex) => ({
    ...entry,
    priority: entryIndex,
  }));
}

export function removeRouterRule(
  rules: RouterRule[],
  ruleId: string,
): RouterRule[] {
  return rules
    .filter((rule) => rule.id !== ruleId)
    .map((rule, index) => ({ ...rule, priority: index }));
}

export function updateRouterRule(
  rules: RouterRule[],
  ruleId: string,
  patch: Partial<RouterRule>,
): RouterRule[] {
  return rules.map((rule) =>
    rule.id === ruleId ? { ...rule, ...patch, id: rule.id } : rule,
  );
}