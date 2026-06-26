#!/usr/bin/env bash
# merge.sh -- Binary-first, shell-fallback front door for the findings merge.
#
# Presents the SAME interface as `parse.sh merge`:
#
#     bash merge.sh <file_a> <file_b> [label_a] [label_b]
#
# and emits the SAME Consensus/Disputes/Blindspots Markdown on stdout. The only
# difference is the engine:
#
#   1. If the native `xavier-tool` binary is present AND compatible, the
#      mechanical exact-match merge runs in the binary (the determinism
#      boundary). This script extracts findings from each model's stream-json,
#      hands them to `xavier-tool merge` as JSON on stdin, and asks the binary
#      to render the debate Markdown directly (`--format debate-md`).
#   2. Otherwise it transparently falls back to `parse.sh merge`, whose output
#      is byte-for-byte what Xavier produced before the binary existed.
#
# A skill must NEVER crash because the binary is missing or misbehaves — every
# failure mode in the binary path degrades to the shell fallback.
#
# Pure Bash glue (grep/awk/sed, same toolchain as parse.sh); the binary path
# additionally uses `xavier-tool` when present.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PARSE_SH="$SCRIPT_DIR/parse.sh"

usage() {
    echo "Usage: merge.sh <file_a> <file_b> [label_a] [label_b]" >&2
    exit 1
}

FILE_A="${1:-}"
FILE_B="${2:-}"
LABEL_A="${3:-Model A}"
LABEL_B="${4:-Model B}"
[ -n "$FILE_A" ] && [ -n "$FILE_B" ] || usage

# ----------------------------------------------------------------------------
# Locate the native tool for this host triple. Mirrors install.sh's layout:
#   $XAVIER_HOME/bin/<triple>/xavier-tool
# Echoes the path, or nothing if unresolved.
# ----------------------------------------------------------------------------
resolve_tool() {
    # Explicit override wins (used by tests and power users).
    if [ -n "${XAVIER_TOOL:-}" ]; then
        [ -x "$XAVIER_TOOL" ] && echo "$XAVIER_TOOL"
        return 0
    fi

    local home triple os arch rust_arch
    home="${XAVIER_HOME:-$HOME/.xavier}"

    os="$(uname -s 2>/dev/null || echo unknown)"
    arch="$(uname -m 2>/dev/null || echo unknown)"
    case "$arch" in
        x86_64|amd64)  rust_arch="x86_64" ;;
        arm64|aarch64) rust_arch="aarch64" ;;
        *)             rust_arch="$arch" ;;
    esac
    case "$os" in
        Darwin) triple="${rust_arch}-apple-darwin" ;;
        Linux)  triple="${rust_arch}-unknown-linux-gnu" ;;
        *)      return 0 ;;
    esac

    local candidate="$home/bin/$triple/xavier-tool"
    [ -x "$candidate" ] && echo "$candidate"
    return 0
}

# ----------------------------------------------------------------------------
# Compatibility probe: the binary must run and advertise a version. A binary
# that cannot print --version is treated as incompatible (fall back).
# ----------------------------------------------------------------------------
tool_compatible() {
    "$1" --version >/dev/null 2>&1
}

# ----------------------------------------------------------------------------
# Shell fallback — exec parse.sh merge. Output is identical to pre-binary
# Xavier. exec preserves exit status and streaming behavior.
# ----------------------------------------------------------------------------
fallback() {
    exec bash "$PARSE_SH" merge "$FILE_A" "$FILE_B" "$LABEL_A" "$LABEL_B"
}

# ----------------------------------------------------------------------------
# Extract findings from a model's stream-json file into the binary's JSON
# Finding[] shape. Reuses parse.sh's `extract` to get the assistant text, then
# parses the `### [severity] desc` / `**File**:` / `**Suggestion**:` blocks the
# same way parse.sh does — emitting JSON instead of TSV.
#
# Prints a JSON array to stdout. Empty/absent input yields `[]`.
# ----------------------------------------------------------------------------
extract_findings_json() {
    local file="$1" source_label="$2" text
    text="$(bash "$PARSE_SH" extract "$file" 2>/dev/null || true)"
    if [ -z "$text" ]; then
        printf '[]'
        return 0
    fi

    printf '%s' "$text" | awk -v source="$source_label" '
        function jesc(v) {
            gsub(/\\/, "\\\\", v); gsub(/"/, "\\\"", v)
            gsub(/\t/, " ", v); gsub(/\r/, "", v)
            return v
        }
        function flush() {
            if (have) {
                if (n > 0) printf ","
                printf "{\"severity\":\"%s\"", jesc(sev)
                if (ref != "") printf ",\"reference\":{\"file\":\"%s\"}", jesc(ref)
                printf ",\"description\":\"%s\"", jesc(desc)
                if (sug != "") printf ",\"suggestion\":\"%s\"", jesc(sug)
                printf ",\"source\":\"%s\"}", jesc(source)
                n++
            }
            have = 0; sev = ""; ref = ""; desc = ""; sug = ""
        }
        BEGIN { printf "["; n = 0; have = 0 }
        /^### \[/ {
            flush()
            line = $0
            sub(/^### \[/, "", line)
            idx = index(line, "] ")
            if (idx > 0) {
                sev = tolower(substr(line, 1, idx - 1))
                desc = substr(line, idx + 2)
            } else {
                sub(/\].*/, "", line); sev = tolower(line)
                desc = $0; sub(/^### \[[^\]]*\] */, "", desc)
            }
            have = 1
            next
        }
        /^\*\*File\*\*:/ {
            line = $0; sub(/^\*\*File\*\*: */, "", line)
            gsub(/`/, "", line); gsub(/^[ \t]+|[ \t]+$/, "", line)
            ref = line; next
        }
        /^\*\*Suggestion\*\*:/ {
            line = $0; sub(/^\*\*Suggestion\*\*: */, "", line)
            gsub(/^[ \t]+|[ \t]+$/, "", line)
            sug = line; next
        }
        END { flush(); printf "]" }
    '
}

# JSON-encode a bare string value (for the labels).
json_str() {
    printf '"%s"' "$(printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g')"
}

# ----------------------------------------------------------------------------
# Main: try the binary path, degrade to shell on ANY problem.
# ----------------------------------------------------------------------------
TOOL="$(resolve_tool || true)"

# No binary, or it failed the compatibility probe -> shell fallback.
if [ -z "$TOOL" ] || ! tool_compatible "$TOOL"; then
    fallback
fi

# Build the merge input from both models' findings.
FA_JSON="$(extract_findings_json "$FILE_A" "$LABEL_A")"
FB_JSON="$(extract_findings_json "$FILE_B" "$LABEL_B")"
INPUT_JSON="$(printf '{"a":%s,"b":%s,"label_a":%s,"label_b":%s}' \
    "$FA_JSON" "$FB_JSON" "$(json_str "$LABEL_A")" "$(json_str "$LABEL_B")")"

# Run the mechanical merge in the binary and let it render the debate Markdown.
# A non-zero exit (input/internal error) or empty output means the binary could
# not do its job — fall back rather than emit nothing.
set +e
RESULT_MD="$(printf '%s' "$INPUT_JSON" | "$TOOL" merge --format debate-md 2>/dev/null)"
TOOL_EXIT=$?
set -e

if [ "$TOOL_EXIT" -ne 0 ] || [ -z "$RESULT_MD" ]; then
    fallback
fi

printf '%s\n' "$RESULT_MD"
