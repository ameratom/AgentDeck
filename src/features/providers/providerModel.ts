import type {
  CredentialStatus,
  ProviderAdapterStatus,
} from "../../lib/types";

export function credentialLabel(status: CredentialStatus): string {
  switch (status) {
    case "not-required":
      return "No key required";
    case "keychain":
      return "Stored in Keychain";
    case "environment":
      return "Development environment";
    case "missing":
      return "API key required";
  }
}

export function providerTargetLabel(provider: ProviderAdapterStatus): string {
  return `${provider.name} - ${credentialLabel(provider.credentialStatus)}`;
}

export function replaceProvider(
  providers: ProviderAdapterStatus[],
  nextProvider: ProviderAdapterStatus,
): ProviderAdapterStatus[] {
  return providers.map((provider) =>
    provider.id === nextProvider.id ? nextProvider : provider,
  );
}
