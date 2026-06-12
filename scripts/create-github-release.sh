#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAG="${1:-v0.1.2}"
DMG_GLOB="$ROOT_DIR/src-tauri/target/release/bundle/dmg/*.dmg"
NOTES_FILE="$ROOT_DIR/RELEASE_NOTES_${TAG}.md"

if ! command -v gh >/dev/null 2>&1; then
  echo "GitHub CLI (gh) is required." >&2
  exit 1
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "Run: gh auth login" >&2
  exit 1
fi

if [[ ! -f "$NOTES_FILE" ]]; then
  echo "Missing release notes: $NOTES_FILE" >&2
  exit 1
fi

DMG_PATH="$(ls -1 $DMG_GLOB 2>/dev/null | head -n 1 || true)"
if [[ -z "$DMG_PATH" ]]; then
  echo "Missing DMG. Run: source scripts/notarize.local.env && pnpm tauri build" >&2
  exit 1
fi

gh release create "$TAG" \
  --repo ameratom/AgentDeck \
  --title "AgentDeck ${TAG}" \
  --notes-file "$NOTES_FILE" \
  "$DMG_PATH"

echo "Release published: https://github.com/ameratom/AgentDeck/releases/tag/${TAG}"