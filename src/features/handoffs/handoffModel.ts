import type {
  HandoffRequest,
  HandoffRun,
  LocalModel,
  ProviderAdapterStatus,
} from "../../lib/types";

const EMBEDDING_MODEL_PATTERN = /(?:embed|embedding)/i;

export function selectDefaultTargetProvider(
  providers: ProviderAdapterStatus[],
): ProviderAdapterStatus | null {
  return (
    providers.find((provider) => provider.id === "lm-studio") ??
    providers[0] ??
    null
  );
}

export function filterChatModels(models: LocalModel[]): LocalModel[] {
  return models.filter((model) => !EMBEDDING_MODEL_PATTERN.test(model.id));
}

export function selectDefaultModel(provider: ProviderAdapterStatus | null): string {
  return selectDefaultModelFromList(provider?.models ?? []);
}

export function selectDefaultModelFromList(models: LocalModel[]): string {
  const chatModels = filterChatModels(models);
  const qwen = chatModels.find((model) => model.id.toLowerCase().includes("qwen"));
  if (qwen) {
    return qwen.id;
  }
  return chatModels[0]?.id ?? models[0]?.id ?? "";
}

export function buildHandoffRequest(args: {
  sourceAgentId: string;
  sourceAgentName: string;
  provider: ProviderAdapterStatus;
  modelId: string;
  title: string;
  task: string;
  context: string;
  approvals: string[];
}): HandoffRequest {
  return buildHandoffRequestFromTarget({
    sourceAgentId: args.sourceAgentId,
    sourceAgentName: args.sourceAgentName,
    targetProviderId: args.provider.id,
    targetProviderName: args.provider.name,
    targetModelId: args.modelId,
    title: args.title,
    task: args.task,
    context: args.context,
    approvals: args.approvals,
  });
}

export function buildHandoffRequestFromTarget(args: {
  sourceAgentId: string;
  sourceAgentName: string;
  targetProviderId: string;
  targetProviderName: string;
  targetModelId: string;
  title: string;
  task: string;
  context: string;
  approvals: string[];
}): HandoffRequest {
  return {
    sourceAgentId: args.sourceAgentId,
    sourceAgentName: args.sourceAgentName,
    targetProviderId: args.targetProviderId,
    targetProviderName: args.targetProviderName,
    targetModelId: args.targetModelId,
    title: args.title.trim(),
    task: args.task.trim(),
    context: args.context.trim(),
    approvals: args.approvals,
  };
}

export function buildApprovalRecord(): string {
  return `user-approved:${new Date().toISOString()}`;
}

export function recentOutput(run: HandoffRun): string {
  return run.output.trim() || run.error?.trim() || "No result captured.";
}
