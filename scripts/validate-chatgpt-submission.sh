#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUBMISSION="$ROOT_DIR/chatgpt-app-submission.json"

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required." >&2
  exit 1
fi

echo "Validating ChatGPT submission: $SUBMISSION"

jq -e . "$SUBMISSION" >/dev/null

schema="$(jq -r '."$schema"' "$SUBMISSION")"
if [[ "$schema" != "https://developers.openai.com/apps-sdk/schemas/chatgpt-app-submission.v1.json" ]]; then
  echo "Unexpected \$schema: $schema" >&2
  exit 1
fi

if [[ "$(jq -r '.schema_version' "$SUBMISSION")" != "1" ]]; then
  echo "schema_version must be 1" >&2
  exit 1
fi

subtitle_len="$(jq -r '.app_info.subtitle' "$SUBMISSION" | wc -c | tr -d ' ')"
subtitle_len=$((subtitle_len - 1))
if [[ "$subtitle_len" -gt 30 ]]; then
  echo "app_info.subtitle exceeds 30 characters ($subtitle_len)" >&2
  exit 1
fi

positive_count="$(jq '.test_cases | length' "$SUBMISSION")"
negative_count="$(jq '.negative_test_cases | length' "$SUBMISSION")"
if [[ "$positive_count" -lt 5 ]]; then
  echo "Need at least 5 test_cases (found $positive_count)" >&2
  exit 1
fi
if [[ "$negative_count" -lt 3 ]]; then
  echo "Need at least 3 negative_test_cases (found $negative_count)" >&2
  exit 1
fi

non_readonly="$(jq -r '
  .tools
  | to_entries[]
  | select(.value.annotations.readOnlyHint != true)
  | .key
' "$SUBMISSION")"
if [[ -n "$non_readonly" ]]; then
  echo "All submitted tools must set readOnlyHint=true:" >&2
  echo "$non_readonly" >&2
  exit 1
fi

missing_annotations="$(jq -r '
  .tools
  | to_entries[]
  | select(
      (.value.annotations.readOnlyHint | type) != "boolean"
      or (.value.annotations.openWorldHint | type) != "boolean"
      or (.value.annotations.destructiveHint | type) != "boolean"
      or (.value.justifications.read_only_justification | length) == 0
      or (.value.justifications.open_world_justification | length) == 0
      or (.value.justifications.destructive_justification | length) == 0
    )
  | .key
' "$SUBMISSION")"
if [[ -n "$missing_annotations" ]]; then
  echo "Tools missing required annotations/justifications:" >&2
  echo "$missing_annotations" >&2
  exit 1
fi

echo "  ✔ JSON structure"
echo "  ✔ subtitle length ($subtitle_len)"
echo "  ✔ $positive_count positive / $negative_count negative test cases"
echo "  ✔ read-only tool profile"
echo
echo "Running Rust alignment test..."
(cd "$ROOT_DIR/src-tauri" && cargo test chatgpt_submission -- --nocapture)

echo
echo "ChatGPT submission validation passed."