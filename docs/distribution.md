# AgentDeck Distribution

AgentDeck is configured to produce a macOS `.app` bundle and `.dmg` through
Tauri.

## Local Bundle Validation

Build the distributable artifacts:

```bash
pnpm tauri build
```

Apply a local ad-hoc hardened-runtime signature for bundle validation:

```bash
./scripts/sign-local-macos.sh
```

Check whether notarization prerequisites are present:

```bash
./scripts/notarize-preflight.sh
```

For Developer ID signing and notarization (requires Apple credentials):

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="TEAMID"
pnpm tauri build
./scripts/notarize-macos.sh
```

Inspect the generated app bundle:

```bash
codesign --verify --deep --strict --verbose=2 src-tauri/target/release/bundle/macos/AgentDeck.app
spctl --assess --type execute --verbose=4 src-tauri/target/release/bundle/macos/AgentDeck.app
```

The `spctl` assessment may reject local unsigned or ad-hoc signed builds. That
is expected until a Developer ID certificate and notarization credentials are
configured.

The generated DMG is a packaging artifact only until the app is signed with a
Developer ID identity and notarized.

## Notarization Requirements

Notarized distribution requires:

- Apple Developer Program membership.
- A Developer ID Application signing identity in the login keychain.
- App Store Connect API credentials or a notarization keychain profile.
- Hardened runtime enabled.

The Tauri config already enables hardened runtime and points at
`src-tauri/entitlements.plist`. The entitlement file intentionally grants no
broad capabilities.

Do not commit signing certificates, private keys, App Store Connect API keys, or
notarization passwords.

## GitHub Actions

The workflow at `.github/workflows/macos-build.yml` runs tests, builds the macOS
`.app` bundle and `.dmg`, and uploads both as artifacts.

Configure these repository secrets to enable signed and notarized builds:

| Secret | Purpose |
|--------|---------|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` Developer ID certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` archive |
| `APPLE_SIGNING_IDENTITY` | Developer ID Application identity name |
| `APPLE_ID` | Apple ID used for notarization |
| `APPLE_PASSWORD` | App-specific password or notary API key |
| `APPLE_TEAM_ID` | Apple Developer Team ID |

When the signing secrets are absent, CI still validates tests and produces an
unsigned local bundle suitable for development.

## Local Notarization

After a signed release build:

```bash
pnpm tauri build
xcrun notarytool submit src-tauri/target/release/bundle/dmg/*.dmg \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait
xcrun stapler staple src-tauri/target/release/bundle/macos/AgentDeck.app
```
