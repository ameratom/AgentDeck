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
  openai: "OpenAI / Codex",
  anthropic: "Anthropic",
  xai: "xAI",
};

export function importOutcomeForProvider(
  providerId: string,
  outcomes: Array<{ slotId: string; status: string; detail: string }>,
): { slotId: string; status: string; detail: string } | null {
  const slotId =
    providerId === "codex" || providerId === "openai-compatible"
      ? "openai"
      : providerId;
  return outcomes.find((outcome) => outcome.slotId === slotId) ?? null;
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

export function providerDispatchBlocked(
  provider: ProviderAdapterStatus | null,
): boolean {
  return (
    provider === null ||
    providerCredentialBlocked(provider) ||
    !provider.verifiedAvailable ||
    provider.models.length === 0
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
