#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="$ROOT_DIR/src-tauri/target/release/bundle/macos/AgentDeck.app"
DMG_GLOB="$ROOT_DIR/src-tauri/target/release/bundle/dmg/"*.dmg

echo "AgentDeck notarization preflight"
echo

missing=0
require() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "  ✘ $name is not set"
    missing=1
  else
    echo "  ✔ $name is set"
  fi
}

echo "Signing identities:"
if security find-identity -v -p codesigning 2>/dev/null | rg -q "Developer ID Application"; then
  security find-identity -v -p codesigning 2>/dev/null | rg "Developer ID Application"
else
  echo "  ✘ No Developer ID Application certificate in login keychain"
  missing=1
fi
echo

echo "Notarization environment:"
if [[ -f "$ROOT_DIR/scripts/notarize.local.env" ]]; then
  # shellcheck disable=SC1091
  source "$ROOT_DIR/scripts/notarize.local.env"
  echo "  ✔ Loaded scripts/notarize.local.env"
fi
require APPLE_SIGNING_IDENTITY
require APPLE_ID
require APPLE_PASSWORD
require APPLE_TEAM_ID
echo

echo "Build artifacts:"
if [[ -d "$APP_BUNDLE" ]]; then
  echo "  ✔ App bundle: $APP_BUNDLE"
  codesign --verify --deep --strict "$APP_BUNDLE" && echo "  ✔ codesign verify passed"
else
  echo "  ✘ Missing app bundle. Run: pnpm tauri build"
  missing=1
fi

DMG_PATH="$(ls -1 $DMG_GLOB 2>/dev/null | head -n 1 || true)"
if [[ -n "$DMG_PATH" ]]; then
  echo "  ✔ DMG: $DMG_PATH"
else
  echo "  ✘ Missing DMG under src-tauri/target/release/bundle/dmg/"
  missing=1
fi
echo

echo "Gatekeeper assessment (expected to fail until notarized):"
spctl --assess --type execute --verbose=4 "$APP_BUNDLE" 2>&1 || true
echo

if [[ "$missing" -eq 0 ]]; then
  echo "Ready for: pnpm tauri build && ./scripts/notarize-macos.sh"
  exit 0
fi

echo "Not ready for notarization. Use ./scripts/sign-local-macos.sh for local ad-hoc signing."
exit 1