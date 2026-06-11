import type { HandoffRequest, HandoffRun, ProviderAdapterStatus } from "../../lib/types";

export function selectDefaultTargetProvider(
  providers: ProviderAdapterStatus[],
): ProviderAdapterStatus | null {
  return (
    providers.find((provider) => provider.id === "lm-studio") ??
    providers[0] ??
    null
  );
}

export function selectDefaultModel(provider: ProviderAdapterStatus | null): string {
  return provider?.models[0]?.id ?? "";
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
  return {
    sourceAgentId: args.sourceAgentId,
    sourceAgentName: args.sourceAgentName,
    targetProviderId: args.provider.id,
    targetProviderName: args.provider.name,
    targetModelId: args.modelId,
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
