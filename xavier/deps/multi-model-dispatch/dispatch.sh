#!/usr/bin/env bash
# dispatch.sh -- Send a prompt to an external model via the Cursor agent CLI.
# Forked from ACE's ace-run.sh, adapted for Xavier multi-model review.
#
# Usage: dispatch.sh <model> <workspace> <output-file> <system-prompt> <user-prompt>
set -euo pipefail

MODEL="${1:?Usage: dispatch.sh <model> <workspace> <output-file> <system-prompt> <user-prompt>}"
WORKSPACE="${2:?Missing workspace path}"
OUTPUT_FILE="${3:?Missing output file path}"
SYSTEM_PROMPT="${4:?Missing system prompt}"
USER_PROMPT="${5:?Missing user prompt}"

TIMEOUT="${XAVIER_TIMEOUT:-1800}"

# Resolve the agent binary
if [[ -n "${XAVIER_AGENT:-}" ]]; then
    AGENT="$XAVIER_AGENT"
elif [[ -x "$HOME/.local/bin/agent" ]]; then
    AGENT="$HOME/.local/bin/agent"
elif command -v agent &>/dev/null; then
    AGENT="$(command -v agent)"
else
    echo "ERROR: agent CLI not found. Install it or set XAVIER_AGENT." >&2
    exit 1
fi

# Validate workspace
if [[ ! -d "$WORKSPACE" ]]; then
    echo "ERROR: workspace directory does not exist: $WORKSPACE" >&2
    exit 1
fi

# Combine system prompt and user prompt into a single prompt string.
# System prompt (persona + vault context) goes first, then the user prompt (diff).
PROMPT="${SYSTEM_PROMPT}

${USER_PROMPT}"

# Run the agent CLI with timeout, capturing output to both the file and stdout.
set +e
timeout "$TIMEOUT" "$AGENT" -p \
    --model "$MODEL" \
    --yolo \
    --workspace "$WORKSPACE" \
    --output-format stream-json \
    "$PROMPT" 2>&1 | tee "$OUTPUT_FILE"

EXIT_CODE=${PIPESTATUS[0]}
set -e

if [[ $EXIT_CODE -eq 124 ]]; then
    echo "WARNING: agent timed out after ${TIMEOUT}s for model $MODEL" >&2
fi

exit $EXIT_CODE
