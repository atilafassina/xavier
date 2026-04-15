#!/usr/bin/env bash
# parse.sh -- Extract and merge structured findings from agent CLI stream-json output.
#
# Pure Bash implementation — no Python, no jq. Uses only standard tools
# (grep, awk, sed) available on macOS and Linux.
#
# Usage:
#     bash parse.sh extract <file>           # Print final assistant text
#     bash parse.sh merge <file_a> <file_b> [label_a] [label_b]
#                                            # Merge findings into debate format
#                                            # Optional labels identify models (default: Model A / Model B)
#
# Trade-off: merge uses exact file:line matching instead of fuzzy description
# similarity. Same location = Consensus, unmatched = Blindspot. This is simpler
# and more predictable, at the cost of occasionally classifying paraphrased-
# but-same findings as two Blindspots instead of one Consensus.

set -euo pipefail

# ============================================================================
# extract_text FILE
# Read stream-json file, print the last assistant text block to stdout.
# ============================================================================
extract_text() {
    local file="$1"
    [[ -f "$file" ]] || { echo "ERROR: File not found: $file" >&2; return 1; }

    # Agent CLI outputs one JSON object per line. We look for assistant
    # messages, find content blocks with "type":"text", and extract the
    # text value. The awk script handles JSON string unescaping.
    grep -E '"type" *: *"assistant"' "$file" 2>/dev/null | awk '
    {
        line = $0
        # Walk through all "type":"text","text":"..." blocks in the line.
        # The content text field follows the type:text declaration.
        while (match(line, /"type" *: *"text" *, *"text" *: *"/)) {
            start = RSTART + RLENGTH
            rest = substr(line, start)
            text = ""

            for (i = 1; i <= length(rest); i++) {
                c = substr(rest, i, 1)
                if (c == "\\") {
                    i++; nc = substr(rest, i, 1)
                    if      (nc == "\"") text = text "\""
                    else if (nc == "n")  text = text "\n"
                    else if (nc == "t")  text = text "\t"
                    else if (nc == "\\") text = text "\\"
                    else if (nc == "/")  text = text "/"
                    else                 text = text "\\" nc
                } else if (c == "\"") {
                    break
                } else {
                    text = text c
                }
            }

            if (length(text) > 0) last = text
            line = substr(rest, i + 1)
        }
    }
    END { if (last != "") printf "%s\n", last; else exit 1 }
    '
}

# ============================================================================
# parse_findings TEXT_FILE OUTPUT_FILE
# Parse ### [severity] description blocks from model markdown into TSV.
# Output: severity<TAB>file_ref<TAB>description<TAB>suggestion
# ============================================================================
parse_findings() {
    local text_file="$1"
    local out_file="$2"

    awk '
    /^### \[/ {
        # Flush previous finding
        if (desc != "") {
            gsub(/\t/, " ", desc); gsub(/\t/, " ", sug)
            printf "%s\t%s\t%s\t%s\n", sev, ref, desc, sug
        }
        # Parse: ### [severity] description
        line = $0
        sub(/^### \[/, "", line)
        idx = index(line, "] ")
        if (idx > 0) {
            sev = tolower(substr(line, 1, idx - 1))
            desc = substr(line, idx + 2)
        } else {
            sev = "unknown"
            sub(/\].*/, "", line)
            sev = tolower(line)
            desc = $0; sub(/^### \[[^\]]*\] */, "", desc)
        }
        ref = ""; sug = ""
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
    END {
        if (desc != "") {
            gsub(/\t/, " ", desc); gsub(/\t/, " ", sug)
            printf "%s\t%s\t%s\t%s\n", sev, ref, desc, sug
        }
    }
    ' "$text_file" > "$out_file"
}

# ============================================================================
# merge_and_format FINDINGS_A FINDINGS_B
# Classify findings into consensus / blindspots, output debate-format markdown.
# Matching is by exact file_ref — no fuzzy similarity.
# ============================================================================
merge_and_format() {
    local fa="$1"
    local fb="$2"
    local label_a="${3:-Model A}"
    local label_b="${4:-Model B}"

    awk -F'\t' -v file_a="$fa" -v label_a="$label_a" -v label_b="$label_b" '
    FILENAME == file_a {
        a_n++
        a_sev[a_n]  = $1; a_ref[a_n]  = $2
        a_desc[a_n] = $3; a_sug[a_n]  = $4
        a_hit[a_n]  = 0
        next
    }
    {
        b_n++
        b_sev[b_n]  = $1; b_ref[b_n]  = $2
        b_desc[b_n] = $3; b_sug[b_n]  = $4
        b_hit[b_n]  = 0
    }
    END {
        # Match by exact file_ref (skip empty refs)
        for (i = 1; i <= a_n; i++) {
            if (a_ref[i] == "") continue
            for (j = 1; j <= b_n; j++) {
                if (b_hit[j]) continue
                if (a_ref[i] == b_ref[j]) {
                    a_hit[i] = j; b_hit[j] = i
                    break
                }
            }
        }

        # --- Consensus ---
        printf "## Consensus\n\n"
        found = 0
        for (i = 1; i <= a_n; i++) {
            if (a_hit[i] == 0) continue
            found = 1; j = a_hit[i]
            sev = a_sev[i]
            if (a_sev[i] != b_sev[j]) sev = a_sev[i] " / " b_sev[j]
            printf "### [%s] %s\n", sev, a_desc[i]
            printf "**File**: %s\n", a_ref[i]
            if (a_sug[i] != "") printf "**Suggestion (%s)**: %s\n", label_a, a_sug[i]
            if (b_sug[j] != "") printf "**Suggestion (%s)**: %s\n", label_b, b_sug[j]
            printf "\n"
        }
        if (!found) print "No consensus findings -- the models did not flag the same locations.\n"
        printf "\n"

        # --- Disputes ---
        # The merge layer does not produce disputes. Disputes are generated
        # exclusively by the pilot fish when vault recurring patterns contradict
        # a consensus finding (see debate.md section 4, vault interaction rules).
        printf "## Disputes\n\n"
        print "No disputes from merge — disputes arise from vault overlay in the pilot fish step.\n"
        printf "\n"

        # --- Blindspots ---
        printf "## Blindspots\n\n"
        found = 0
        for (i = 1; i <= a_n; i++) {
            if (a_hit[i] != 0) continue
            found = 1
            printf "### [%s] %s\n", a_sev[i], a_desc[i]
            if (a_ref[i] != "") printf "**File**: %s\n", a_ref[i]
            printf "**Source**: %s only\n", label_a
            if (a_sug[i] != "") printf "**Suggestion**: %s\n", a_sug[i]
            printf "\n"
        }
        for (j = 1; j <= b_n; j++) {
            if (b_hit[j] != 0) continue
            found = 1
            printf "### [%s] %s\n", b_sev[j], b_desc[j]
            if (b_ref[j] != "") printf "**File**: %s\n", b_ref[j]
            printf "**Source**: %s only\n", label_b
            if (b_sug[j] != "") printf "**Suggestion**: %s\n", b_sug[j]
            printf "\n"
        }
        if (!found) print "No blindspots -- both models covered the same ground.\n"
        printf "\n"
    }
    ' "$fa" "$fb"
}

# ============================================================================
# CLI
# ============================================================================
case "${1:-}" in
    extract)
        [[ -n "${2:-}" ]] || { echo "Usage: parse.sh extract <file>" >&2; exit 1; }
        extract_text "$2"
        ;;
    merge)
        [[ -n "${2:-}" ]] && [[ -n "${3:-}" ]] || {
            echo "Usage: parse.sh merge <file_a> <file_b> [label_a] [label_b]" >&2; exit 1
        }

        LABEL_A="${4:-Model A}"
        LABEL_B="${5:-Model B}"

        tmp_dir=$(mktemp -d)
        trap 'rm -f "$tmp_dir"/* 2>/dev/null; rmdir "$tmp_dir" 2>/dev/null || true' EXIT

        text_a="$tmp_dir/text_a.md"
        text_b="$tmp_dir/text_b.md"
        extract_text "$2" > "$text_a" 2>/dev/null || true
        extract_text "$3" > "$text_b" 2>/dev/null || true

        if [[ ! -s "$text_a" ]] && [[ ! -s "$text_b" ]]; then
            echo "ERROR: no assistant text found in either file." >&2
            exit 1
        fi

        findings_a="$tmp_dir/findings_a.tsv"
        findings_b="$tmp_dir/findings_b.tsv"
        [[ -s "$text_a" ]] && parse_findings "$text_a" "$findings_a" || touch "$findings_a"
        [[ -s "$text_b" ]] && parse_findings "$text_b" "$findings_b" || touch "$findings_b"

        merge_and_format "$findings_a" "$findings_b" "$LABEL_A" "$LABEL_B"
        ;;
    *)
        echo "Usage: parse.sh {extract|merge} <args...>" >&2
        echo "" >&2
        echo "Commands:" >&2
        echo "  extract <file>           Print final assistant text from stream-json" >&2
        echo "  merge <file_a> <file_b> [label_a] [label_b]" >&2
        echo "                           Merge findings into debate format" >&2
        echo "                           Labels identify models (default: Model A / Model B)" >&2
        exit 1
        ;;
esac
