#!/usr/bin/env bash
# merge.sh -- Binary-first, shell-fallback front door for the findings merge.
#
# Presents the SAME interface as `parse.sh merge`:
#
#     bash merge.sh <file_a> <file_b> [label_a] [label_b]
#
# and emits Consensus/Disputes/Blindspots Markdown on stdout (the binary path
# additionally emits an `## Unmatched` section for the model to adjudicate; the
# pilot fish detects debate format by the first three headings, so this is
# additive). The difference between the two engines:
#
#   1. If the native `xavier-tool` binary is present AND supports the
#      `merge-text` subcommand, the mechanical merge runs in the binary (the
#      determinism boundary). This script extracts each model's assistant TEXT
#      from its stream-json with `parse.sh extract`, hands the raw text to
#      `xavier-tool merge-text` as JSON on stdin, and asks the binary to render
#      the debate Markdown directly (`--format debate-md`). The binary parses
#      the findings out of the Markdown itself (multi-line descriptions,
#      \uXXXX escapes, non-strict formatting) and does exact + textual
#      near-duplicate matching — fixing the paraphrase-splitting that the awk
#      parser and exact-only matcher suffered from.
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
    # Architecture normalization mirrors install.sh detect_host_triple():
    # anything outside the shipped {x86_64, aarch64} set yields no triple, so we
    # never probe a bin/<triple>/ path we never built.
    case "$arch" in
        x86_64|amd64)  rust_arch="x86_64" ;;
        arm64|aarch64) rust_arch="aarch64" ;;
        *)             return 0 ;;
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
# Compatibility probe: the binary must run, advertise a version, AND support the
# `merge-text` subcommand this front door now drives. We verify the capability
# directly by feeding an empty MergeTextInput (`{}`, valid via serde defaults)
# and requiring a clean exit. `--no-cache` keeps the probe pure — a capability
# check must never read or write the on-disk result cache. An older binary that
# predates `merge-text` exits non-zero ("unknown subcommand" -> exit 2), so it is
# treated as incompatible and we fall back to parse.sh. This keeps the version
# probe and the capability probe in agreement without parsing version strings.
# ----------------------------------------------------------------------------
tool_compatible() {
    "$1" --version >/dev/null 2>&1 || return 1
    printf '{}' | "$1" merge-text --no-cache >/dev/null 2>&1
}

# ----------------------------------------------------------------------------
# Shell fallback — exec parse.sh merge. Output is identical to pre-binary
# Xavier. exec preserves exit status and streaming behavior.
# ----------------------------------------------------------------------------
fallback() {
    exec bash "$PARSE_SH" merge "$FILE_A" "$FILE_B" "$LABEL_A" "$LABEL_B"
}

# ----------------------------------------------------------------------------
# JSON-encode a string value. Handles the control characters that can appear in
# extracted assistant text (newlines, tabs, CR) plus quotes and backslashes, so
# the result is a valid JSON string literal. The binary's parser owns the
# Markdown -> findings step, so this is the ONLY encoding the shell now does.
# ----------------------------------------------------------------------------
json_str() {
    printf '%s' "$1" | awk '
        BEGIN { ORS=""; printf "\"" }
        {
            if (NR > 1) printf "\\n"   # restore the newline awk consumed
            line = $0
            gsub(/\\/, "\\\\", line)
            gsub(/"/,  "\\\"", line)
            gsub(/\t/, "\\t",  line)
            gsub(/\r/, "\\r",  line)
            printf "%s", line
        }
        END { printf "\"" }
    '
}

# ----------------------------------------------------------------------------
# Main: try the binary path, degrade to shell on ANY problem.
# ----------------------------------------------------------------------------
TOOL="$(resolve_tool || true)"

# No binary, or it failed the compatibility probe -> shell fallback.
if [ -z "$TOOL" ] || ! tool_compatible "$TOOL"; then
    fallback
fi

# Extract each model's final assistant TEXT (stream-json -> Markdown). The
# binary parses findings out of that Markdown itself via `merge-text`.
TEXT_A="$(bash "$PARSE_SH" extract "$FILE_A" 2>/dev/null || true)"
TEXT_B="$(bash "$PARSE_SH" extract "$FILE_B" 2>/dev/null || true)"

# As in parse.sh, if neither model produced any text there is nothing to merge.
if [ -z "$TEXT_A" ] && [ -z "$TEXT_B" ]; then
    fallback
fi

# Build the MergeTextInput payload (raw text per side + labels).
INPUT_JSON="$(printf '{"text_a":%s,"text_b":%s,"label_a":%s,"label_b":%s}' \
    "$(json_str "$TEXT_A")" "$(json_str "$TEXT_B")" \
    "$(json_str "$LABEL_A")" "$(json_str "$LABEL_B")")"

# Run the mechanical merge in the binary and let it render the debate Markdown.
# A non-zero exit (input/internal error) or empty output means the binary could
# not do its job — fall back rather than emit nothing.
set +e
RESULT_MD="$(printf '%s' "$INPUT_JSON" | "$TOOL" merge-text --format debate-md 2>/dev/null)"
TOOL_EXIT=$?
set -e

if [ "$TOOL_EXIT" -ne 0 ] || [ -z "$RESULT_MD" ]; then
    fallback
fi

printf '%s\n' "$RESULT_MD"
