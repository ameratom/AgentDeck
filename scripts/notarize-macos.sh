#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="$ROOT_DIR/src-tauri/target/release/bundle/macos/AgentDeck.app"
DMG_GLOB="$ROOT_DIR/src-tauri/target/release/bundle/dmg/*.dmg"

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

DMG_PATH="$(ls -1t "$ROOT_DIR"/src-tauri/target/release/bundle/dmg/AgentDeck_*.dmg 2>/dev/null | rg -v '/rw\.' | head -n 1 || true)"
if [[ -z "$DMG_PATH" ]]; then
  DMG_PATH="$(ls -1 $DMG_GLOB 2>/dev/null | rg -v '/rw\.' | head -n 1 || true)"
fi
if [[ -z "$DMG_PATH" ]]; then
  echo "Missing DMG artifact under src-tauri/target/release/bundle/dmg/" >&2
  exit 1
fi

echo "Submitting $DMG_PATH for notarization..."
SUBMIT_OUTPUT="$(xcrun notarytool submit "$DMG_PATH" \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" 2>&1)"
echo "$SUBMIT_OUTPUT"
SUBMISSION_ID="$(printf '%s\n' "$SUBMIT_OUTPUT" | awk '/^[[:space:]]*id:/{print $2; exit}')"
if [[ -z "$SUBMISSION_ID" ]]; then
  echo "Failed to parse notarization submission id" >&2
  exit 1
fi

echo "Waiting for notarization ($SUBMISSION_ID)..."
for attempt in $(seq 1 60); do
  INFO_OUTPUT="$(xcrun notarytool info "$SUBMISSION_ID" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" 2>&1)"
  STATUS="$(printf '%s\n' "$INFO_OUTPUT" | sed -n 's/^[[:space:]]*status:[[:space:]]*//p' | head -n 1)"
  echo "  poll $attempt: ${STATUS:-unknown}"
  case "$STATUS" in
    Accepted)
      break
      ;;
    Invalid|Rejected)
      echo "$INFO_OUTPUT" >&2
      xcrun notarytool log "$SUBMISSION_ID" \
        --apple-id "$APPLE_ID" \
        --password "$APPLE_PASSWORD" \
        --team-id "$APPLE_TEAM_ID" >&2 || true
      exit 1
      ;;
  esac
  sleep 20
done

if [[ "${STATUS:-}" != "Accepted" ]]; then
  echo "Notarization timed out for submission $SUBMISSION_ID" >&2
  exit 1
fi

echo "Stapling notarization ticket to app bundle..."
xcrun stapler staple "$APP_BUNDLE"

echo "Verifying app bundle signature..."
codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"
spctl --assess --type execute --verbose=4 "$APP_BUNDLE" || true

echo "Notarization complete:"
echo "  App: $APP_BUNDLE"
echo "  DMG: $DMG_PATH"