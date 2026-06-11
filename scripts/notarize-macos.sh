#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="$ROOT_DIR/src-tauri/target/release/bundle/macos/AgentDeck.app"
DMG_GLOB="$ROOT_DIR/src-tauri/target/release/bundle/dmg/"*.dmg"

if [[ -f "$ROOT_DIR/scripts/notarize.local.env" ]]; then
  # shellcheck disable=SC1091
  source "$ROOT_DIR/scripts/notarize.local.env"
fi

require_env() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "Missing required environment variable: $name" >&2
    exit 1
  fi
}

if [[ ! -d "$APP_BUNDLE" ]]; then
  echo "Missing app bundle: $APP_BUNDLE" >&2
  echo "Run pnpm tauri build first." >&2
  exit 1
fi

require_env APPLE_ID
require_env APPLE_PASSWORD
require_env APPLE_TEAM_ID

DMG_PATH="$(ls -1 $DMG_GLOB 2>/dev/null | head -n 1 || true)"
if [[ -z "$DMG_PATH" ]]; then
  echo "Missing DMG artifact under src-tauri/target/release/bundle/dmg/" >&2
  exit 1
fi

echo "Submitting $DMG_PATH for notarization..."
xcrun notarytool submit "$DMG_PATH" \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait

echo "Stapling notarization ticket to app bundle..."
xcrun stapler staple "$APP_BUNDLE"

echo "Verifying app bundle signature..."
codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"
spctl --assess --type execute --verbose=4 "$APP_BUNDLE" || true

echo "Notarization complete:"
echo "  App: $APP_BUNDLE"
echo "  DMG: $DMG_PATH"