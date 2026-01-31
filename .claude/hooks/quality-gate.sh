#!/bin/bash
set -euo pipefail

INPUT=$(cat)
CWD=$(echo "$INPUT" | jq -r '.cwd')
STOP_HOOK_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active // false')

# Prevent infinite loop - if we already ran, let Claude stop
if [ "$STOP_HOOK_ACTIVE" = "true" ]; then
  exit 0
fi

cd "$CWD"

# Run quality checks
if just check > /tmp/sp-quality.log 2>&1; then
  exit 0  # All good, Claude can stop
else
  # Extract last 30 lines of failure output
  ERRORS=$(tail -30 /tmp/sp-quality.log | jq -Rs .)
  echo "{\"decision\":\"block\",\"reason\":\"Quality gate failed. Fix issues:\\n\\n\"$ERRORS\"\"}"
  exit 0
fi
