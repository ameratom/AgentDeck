#!/usr/bin/env bash
set -euo pipefail

if [[ $# -gt 0 ]]; then
  ROOTS=("$@")
elif [[ -n "${AGENTDECK_FS_ROOTS:-}" ]]; then
  IFS=':' read -r -a ROOTS <<< "$AGENTDECK_FS_ROOTS"
else
  ROOT="$(cd "${AGENTDECK_PROJECT_ROOT:-$(pwd)}" && pwd)"
  ROOTS=("$ROOT")
fi

RESOLVED=()
for root in "${ROOTS[@]}"; do
  if [[ -z "$root" ]]; then
    continue
  fi
  if [[ ! -e "$root" ]]; then
    echo "Filesystem MCP: path does not exist: $root" >&2
    exit 1
  fi
  RESOLVED+=("$(cd "$root" && pwd)")
done

if [[ ${#RESOLVED[@]} -eq 0 ]]; then
  echo "Filesystem MCP: no allowed roots configured." >&2
  echo "Set AGENTDECK_PROJECT_ROOT, AGENTDECK_FS_ROOTS, or pass paths as arguments." >&2
  exit 1
fi

# MVP policy: scope to explicit project roots only. Deny secret paths in prompts.
for root in "${RESOLVED[@]}"; do
  case "$root" in
    *"/.env" | *"/.env/"* | *"/secret.key" | *"/provider_secrets"*)
      echo "Filesystem MCP: refusing sensitive path: $root" >&2
      exit 1
      ;;
  esac
done

exec npx -y @modelcontextprotocol/server-filesystem "${RESOLVED[@]}"