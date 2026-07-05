#!/usr/bin/env bash
set -euo pipefail

# validate-shell-degradation.sh
#
# Backpressure guard for the pure-shell fallback merge (parse.sh via merge.sh
# with the binary forced off). Mirrors the style of validate-skills.sh /
# validate-install-triples.sh: pure bash assertions, no network, builds nothing,
# echoes PASS/FAIL per check and "ALL CHECKS PASSED" at the end (non-zero exit on
# any failure).
#
# WHAT IT PROTECTS
# The shell fallback used to SILENTLY DROP findings whenever a model answered in
# prose instead of the rigid "### [severity]" + "**File**:" format: parse.sh's
# awk only fired on /^### \[/, and merge_and_format had no ## Unmatched bucket, so
# a prose review produced empty Consensus/Disputes/Blindspots and the finding
# vanished. The native xavier-tool binary never lost it (it defers such findings
# to ## Unmatched). This harness locks in the "dumb-but-honest shell" contract:
#
#   1. The shell fallback renders the SAME section SET as the binary
#      (Consensus / Disputes / Blindspots / Unmatched).
#   2. A prose-only review (no rigid heading) is NOT dropped — it is deferred to
#      ## Unmatched, and its distinctive text survives.
#   3. Conforming input (### [sev] + **File**:) still yields the classic sections
#      unchanged (matching refs -> Consensus).
#
# This is DEFER-ONLY parity: "same rendered section set + shell never drops", not
# "same findings". The shell does not replicate the binary's parsing intelligence.
#
# Fixtures are committed under xavier/deps/multi-model-dispatch/testdata/ so the
# test is self-contained and CI-safe (it does NOT depend on any ~/.xavier path).

ERRORS=0
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MERGE_SH="$REPO_ROOT/xavier/deps/multi-model-dispatch/merge.sh"
TESTDATA="$REPO_ROOT/xavier/deps/multi-model-dispatch/testdata"

# The two prose-format review fixtures that used to be silently dropped.
PROSE_A="$TESTDATA/sec_gpt.json"
PROSE_B="$TESTDATA/sec_gemini.json"

# A distinctive token from the GPT prose finding — its presence proves the
# finding text survived rather than being dropped.
DISTINCTIVE_TOKEN="CWE-1427"

for f in "$MERGE_SH" "$PROSE_A" "$PROSE_B"; do
  if [ ! -f "$f" ]; then
    echo "FAIL: required file not found: $f"
    exit 1
  fi
done

# Run the SHELL fallback (binary forced off via the kill switch).
run_shell_merge() {
  XAVIER_TOOL_DISABLE=1 bash "$MERGE_SH" "$1" "$2" "${3:-GPT}" "${4:-Gemini}" 2>/dev/null
}

# ----------------------------------------------------------------------------
# (1) Shell fallback emits the ## Unmatched section on the prose fixtures.
# ----------------------------------------------------------------------------
echo "=== Checking shell fallback emits ## Unmatched on prose input ==="
PROSE_OUT="$(run_shell_merge "$PROSE_A" "$PROSE_B" GPT Gemini)"

if printf '%s\n' "$PROSE_OUT" | grep -qE '^## Unmatched$'; then
  echo "PASS: ## Unmatched section is emitted"
else
  echo "FAIL: ## Unmatched section is MISSING from shell fallback output"
  ERRORS=$((ERRORS + 1))
fi

# ----------------------------------------------------------------------------
# (2) Nothing silently dropped: the prose finding's distinctive text is present,
#     and specifically lands under ## Unmatched (not vanished).
# ----------------------------------------------------------------------------
echo ""
echo "=== Checking the prose finding is not silently dropped ==="

if printf '%s\n' "$PROSE_OUT" | grep -qF "$DISTINCTIVE_TOKEN"; then
  echo "PASS: distinctive finding token ('$DISTINCTIVE_TOKEN') survived in output"
else
  echo "FAIL: distinctive finding token ('$DISTINCTIVE_TOKEN') was DROPPED"
  ERRORS=$((ERRORS + 1))
fi

# Slice the ## Unmatched section (from its header to the next '## ' or EOF) and
# assert it actually carries a finding (a '### [' entry), not the empty-state
# string. This proves the finding was routed to Unmatched, not dropped.
UNMATCHED_SECTION="$(printf '%s\n' "$PROSE_OUT" | awk '
  /^## Unmatched$/ { cap=1; next }
  cap && /^## / { cap=0 }
  cap { print }
')"

if printf '%s\n' "$UNMATCHED_SECTION" | grep -qE '^### \['; then
  echo "PASS: ## Unmatched section carries at least one finding entry"
else
  echo "FAIL: ## Unmatched section is empty (finding not routed there)"
  ERRORS=$((ERRORS + 1))
fi

if printf '%s\n' "$UNMATCHED_SECTION" | grep -qF "$DISTINCTIVE_TOKEN"; then
  echo "PASS: the dropped-before finding now appears under ## Unmatched"
else
  echo "FAIL: finding token not found under ## Unmatched"
  ERRORS=$((ERRORS + 1))
fi

# ----------------------------------------------------------------------------
# (3) Section-set parity with the binary: the shell fallback must render exactly
#     the four-section set the binary renders (Consensus/Disputes/Blindspots/
#     Unmatched), in that order.
# ----------------------------------------------------------------------------
echo ""
echo "=== Checking shell fallback section set matches the binary's four ==="
GOT_SECTIONS="$(printf '%s\n' "$PROSE_OUT" | grep -E '^## ' || true)"
WANT_SECTIONS="$(printf '## %s\n' Consensus Disputes Blindspots Unmatched)"

if [ "$GOT_SECTIONS" = "$WANT_SECTIONS" ]; then
  echo "PASS: section set is exactly Consensus / Disputes / Blindspots / Unmatched"
else
  echo "FAIL: section set does not match the expected four"
  echo "  got:"
  printf '    %s\n' "$GOT_SECTIONS"
  echo "  want:"
  printf '    %s\n' "$WANT_SECTIONS"
  ERRORS=$((ERRORS + 1))
fi

# ----------------------------------------------------------------------------
# (4) Conforming input is unchanged: two well-formed findings that share a
#     **File** ref must still merge into Consensus (proving the Unmatched routing
#     did not disturb the classic path). Built in a throwaway sandbox so the test
#     is self-contained; cleanup never touches anything outside the mktemp dir.
# ----------------------------------------------------------------------------
echo ""
echo "=== Checking conforming input still yields the classic sections ==="

SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/xavier-degradation.XXXXXX")"
cleanup() { [ -n "${SANDBOX:-}" ] && [ -d "$SANDBOX" ] && rm -rf "$SANDBOX"; }
trap cleanup EXIT

CONF_A="$SANDBOX/conf_a.json"
CONF_B="$SANDBOX/conf_b.json"
# Both flag the same file:line in the rigid format -> exact-ref match -> Consensus.
printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"### [high] SQL injection in query builder\n**File**: src/db/query.rs:42\n**Suggestion**: Use parameterized queries.\n"}]}}' > "$CONF_A"
printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"### [critical] Unsanitized input reaches SQL\n**File**: src/db/query.rs:42\n**Suggestion**: Bind params.\n"}]}}' > "$CONF_B"

CONF_OUT="$(run_shell_merge "$CONF_A" "$CONF_B" GPT Gemini)"

# Consensus must be populated (the classic behavior), not the empty-state string.
CONSENSUS_SECTION="$(printf '%s\n' "$CONF_OUT" | awk '
  /^## Consensus$/ { cap=1; next }
  cap && /^## / { cap=0 }
  cap { print }
')"

if printf '%s\n' "$CONSENSUS_SECTION" | grep -qE '^### \['; then
  echo "PASS: conforming matched-ref input still produces a Consensus finding"
else
  echo "FAIL: conforming input no longer produces Consensus (classic path broke)"
  ERRORS=$((ERRORS + 1))
fi

# The **File** ref must carry through unchanged in the merged finding.
if printf '%s\n' "$CONSENSUS_SECTION" | grep -qF 'src/db/query.rs:42'; then
  echo "PASS: conforming finding's **File** location is preserved in Consensus"
else
  echo "FAIL: conforming finding's **File** location was lost"
  ERRORS=$((ERRORS + 1))
fi

# And conforming input must NOT spill into Unmatched (every finding was located).
CONF_UNMATCHED="$(printf '%s\n' "$CONF_OUT" | awk '
  /^## Unmatched$/ { cap=1; next }
  cap && /^## / { cap=0 }
  cap { print }
')"
if printf '%s\n' "$CONF_UNMATCHED" | grep -qE '^### \['; then
  echo "FAIL: conforming (located) findings leaked into ## Unmatched"
  ERRORS=$((ERRORS + 1))
else
  echo "PASS: conforming located findings stayed out of ## Unmatched"
fi

echo ""
if [ $ERRORS -gt 0 ]; then
  echo "SHELL DEGRADATION: $ERRORS error(s) found"
  exit 1
else
  echo "ALL CHECKS PASSED"
  exit 0
fi
