import type {
  ProviderAdapterStatus,
  RouterEvaluation,
  RouterRule,
} from "../../lib/types";

export interface RouterInput {
  task: string;
  context: string;
  sourceAgentId: string;
  providers: ProviderAdapterStatus[];
  rules: RouterRule[];
}

export function evaluateRouter(input: RouterInput): RouterEvaluation {
  const taskSize = input.task.length + input.context.length;
  const haystack = `${input.task} ${input.context}`.toLowerCase();
  const sorted = [...input.rules].sort(
    (left, right) => left.priority - right.priority,
  );

  for (const rule of sorted) {
    if (!matchesRule(rule, input.sourceAgentId, haystack, taskSize)) {
      continue;
    }

    const provider = input.providers.find(
      (candidate) => candidate.id === rule.route.providerId,
    );
    if (!provider) {
      continue;
    }
    if (
      provider.authMode !== "none" &&
      provider.credentialStatus === "missing"
    ) {
      continue;
    }

    const modelId =
      rule.route.modelId ||
      provider.models.find((model) => !model.id.toLowerCase().includes("embed"))
        ?.id ||
      provider.models[0]?.id ||
      "";

    return {
      rule,
      providerId: provider.id,
      modelId,
      warning: rule.warnLargeTask
        ? `Task is ${taskSize} characters. Review before dispatching.`
        : null,
    };
  }

  const fallback =
    input.providers.find((provider) => provider.id === "lm-studio") ??
    input.providers[0] ??
    null;

  return {
    rule: null,
    providerId: fallback?.id ?? null,
    modelId: fallback?.models[0]?.id ?? null,
    warning: null,
  };
}

function matchesRule(
  rule: RouterRule,
  sourceAgentId: string,
  haystack: string,
  taskSize: number,
): boolean {
  const { matchRules } = rule;
  if (
    matchRules.sourceAgent &&
    matchRules.sourceAgent !== sourceAgentId
  ) {
    return false;
  }
  if (
    matchRules.taskSizeGt !== undefined &&
    taskSize <= matchRules.taskSizeGt
  ) {
    return false;
  }
  if (matchRules.keywords?.length) {
    const keywordMatch = matchRules.keywords.some((keyword) =>
      haystack.includes(keyword.toLowerCase()),
    );
    if (!keywordMatch) {
      return false;
    }
  }
  return true;
}