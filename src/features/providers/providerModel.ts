import type {
  CredentialStatus,
  ProviderAdapterStatus,
} from "../../lib/types";

export function credentialLabel(status: CredentialStatus): string {
  switch (status) {
    case "not-required":
      return "No key required";
    case "stored":
      return "Stored (encrypted)";
    case "environment":
      return "Development environment";
    case "unreadable":
      return "Unreadable — re-save or import";
    case "import-failed":
      return "Import failed — approve Keychain or enter key";
    case "missing":
      return "Not configured — add or import key";
  }
}

export function credentialStatusClass(status: CredentialStatus): string {
  switch (status) {
    case "unreadable":
    case "import-failed":
      return `credential-status ${status}`;
    case "missing":
      return "credential-status missing";
    case "stored":
      return "credential-status stored";
    case "environment":
      return "credential-status environment";
    default:
      return "credential-status";
  }
}

const SLOT_LABELS: Record<string, string> = {
  openai: "OpenAI API",
  anthropic: "Anthropic API",
  xai: "xAI",
};

const CLI_PROVIDER_IDS = new Set(["claude-code", "codex"]);

export function importOutcomeForProvider(
  providerId: string,
  outcomes: Array<{ slotId: string; status: string; detail: string }>,
): { slotId: string; status: string; detail: string } | null {
  const slotId = providerId === "openai-compatible" ? "openai" : providerId;
  return outcomes.find((outcome) => outcome.slotId === slotId) ?? null;
}

export function providerUsesCliSession(provider: ProviderAdapterStatus): boolean {
  return provider.authMode === "none" && CLI_PROVIDER_IDS.has(provider.id);
}

export function providerReadyForChat(
  provider: ProviderAdapterStatus,
): boolean {
  return (
    !providerCredentialBlocked(provider) && providerHasDispatchableModels(provider)
  );
}

export function importOutcomeLabel(slotId: string): string {
  return SLOT_LABELS[slotId] ?? slotId;
}

export function providerTargetLabel(provider: ProviderAdapterStatus): string {
  return `${provider.name} - ${credentialLabel(provider.credentialStatus)}`;
}

export function providerCredentialBlocked(
  provider: ProviderAdapterStatus | null,
): boolean {
  return (
    provider !== null &&
    provider.authMode !== "none" &&
    (provider.credentialStatus === "missing" ||
      provider.credentialStatus === "import-failed" ||
      provider.credentialStatus === "unreadable")
  );
}

export function providerHasDispatchableModels(
  provider: ProviderAdapterStatus,
): boolean {
  return (
    provider.models.length > 0 &&
    (provider.verifiedAvailable ||
      provider.catalogSource === "static" ||
      provider.catalogSource === "fallback")
  );
}

export function providerDispatchBlocked(
  provider: ProviderAdapterStatus | null,
): boolean {
  return (
    provider === null ||
    providerCredentialBlocked(provider) ||
    !providerHasDispatchableModels(provider)
  );
}

export function replaceProvider(
  providers: ProviderAdapterStatus[],
  nextProvider: ProviderAdapterStatus,
): ProviderAdapterStatus[] {
  return providers.map((provider) =>
    provider.id === nextProvider.id ? nextProvider : provider,
  );
}
