#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="$ROOT_DIR/src-tauri/target/release/bundle/macos/AgentDeck.app"
DMG_PATH="$(ls -1t "$ROOT_DIR"/src-tauri/target/release/bundle/dmg/AgentDeck_*.dmg 2>/dev/null | rg -v '/rw\.' | head -n 1 || true)"

if [[ -f "$ROOT_DIR/scripts/notarize.local.env" ]]; then
  # shellcheck disable=SC1091
  source "$ROOT_DIR/scripts/notarize.local.env"
fi

SUBMISSION_ID="${1:-}"
if [[ -z "$SUBMISSION_ID" ]]; then
  echo "usage: $0 <notarization-submission-id>" >&2
  exit 1
fi

for attempt in $(seq 1 60); do
  INFO_OUTPUT="$(xcrun notarytool info "$SUBMISSION_ID" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" 2>&1)"
  STATUS="$(printf '%s\n' "$INFO_OUTPUT" | sed -n 's/^[[:space:]]*status:[[:space:]]*//p' | head -n 1)"
  echo "poll $attempt: ${STATUS:-unknown}"
  case "$STATUS" in
    Accepted)
      echo "Stapling app bundle..."
      xcrun stapler staple "$APP_BUNDLE"
      if [[ -n "$DMG_PATH" && -f "$DMG_PATH" ]]; then
        echo "Stapling DMG..."
        xcrun stapler staple "$DMG_PATH"
      fi
      codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"
      spctl --assess --type execute --verbose=4 "$APP_BUNDLE" || true
      echo "Staple complete."
      exit 0
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

echo "Timed out waiting for submission $SUBMISSION_ID" >&2
exit 1