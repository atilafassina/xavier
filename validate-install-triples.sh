#!/bin/bash
set -euo pipefail

# validate-install-triples.sh
#
# Offline guard for the install-time platform → target-triple mapping and the
# graceful no-stub fallback. Mirrors the style of validate-skills.sh /
# validate-xavier-frontmatter.sh: pure bash, no network, builds nothing.
#
# It exercises the REAL functions from xavier/install.sh by extracting their
# bodies and evaluating them under stubs, so this test stays honest to the
# shipped source instead of re-encoding behavior. What is checked:
#   (a) every supported uname -s/-m pair maps to the exact triple we ship,
#   (b) unsupported pairs (bad arch and bad OS) map to the empty string,
#   (c) select_native_tool() no-ops (NO stub written) when no bundled binary
#       matches the host triple — i.e. it falls back to the pure-shell merge.
#   (d) XAVIER_TOOL_DISABLE kill switch forces merge.sh's resolve_tool() empty.
#   (e) link_or_replace() replaces a real-dir destination with a symlink (moving
#       it aside to .prev) instead of nesting the link inside it — the clone-mode
#       linker bug that made an install.sh refresh silently no-op on a
#       tarball-installed vault (real dirs, not symlinks).
#
# The supported set MUST match .github/workflows/release.yml's build matrix and
# merge.sh's resolve_tool(): {x86_64,aarch64} x {apple-darwin, unknown-linux-gnu}.

ERRORS=0
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_SH="$REPO_ROOT/xavier/install.sh"
MERGE_SH="$REPO_ROOT/xavier/deps/multi-model-dispatch/merge.sh"

if [ ! -f "$INSTALL_SH" ]; then
  echo "FAIL: $INSTALL_SH not found"
  exit 1
fi
if [ ! -f "$MERGE_SH" ]; then
  echo "FAIL: $MERGE_SH not found"
  exit 1
fi

# ----------------------------------------------------------------------------
# Extract a shell function definition (from `name() {` to its closing `}` at
# column 0) out of a script. This lets us eval the live source without
# triggering the script's top-level `main`/`set -eu` matter.
# ----------------------------------------------------------------------------
extract_fn() {
  src="$1"; fn_name="$2"
  awk -v fn="$fn_name" '
    $0 ~ "^" fn "\\(\\) \\{" { capture=1 }
    capture { print }
    capture && /^\}$/ { exit }
  ' "$src"
}

DETECT_FN="$(extract_fn "$INSTALL_SH" detect_host_triple)"
SELECT_FN="$(extract_fn "$INSTALL_SH" select_native_tool)"
RESOLVE_FN="$(extract_fn "$MERGE_SH" resolve_tool)"
LINK_FN="$(extract_fn "$INSTALL_SH" link_or_replace)"

if [ -z "$DETECT_FN" ]; then
  echo "FAIL: could not extract detect_host_triple() from install.sh"
  exit 1
fi
if [ -z "$SELECT_FN" ]; then
  echo "FAIL: could not extract select_native_tool() from install.sh"
  exit 1
fi
if [ -z "$RESOLVE_FN" ]; then
  echo "FAIL: could not extract resolve_tool() from merge.sh"
  exit 1
fi
if [ -z "$LINK_FN" ]; then
  echo "FAIL: could not extract link_or_replace() from install.sh"
  exit 1
fi

# ----------------------------------------------------------------------------
# Run detect_host_triple() with a stubbed `uname`. We override `uname` as a
# shell function in a subshell, eval the extracted definition, and capture the
# echoed triple. Args: <uname -s value> <uname -m value>.
# ----------------------------------------------------------------------------
run_detect() {
  _os="$1"
  _arch="$2"
  (
    uname() {
      case "${1:-}" in
        -s) printf '%s\n' "$_os" ;;
        -m) printf '%s\n' "$_arch" ;;
        *)  printf '%s\n' "unknown" ;;
      esac
    }
    eval "$DETECT_FN"
    detect_host_triple
  )
}

# ----------------------------------------------------------------------------
# (a) Supported pairs → exact expected triple.
#     Drives the full uname -s/-m matrix, including the amd64/arm64 aliases
#     that BSD/other unames report, since detect_host_triple normalizes them.
# ----------------------------------------------------------------------------
echo "=== Checking supported uname → triple mapping ==="

# Format: "<uname -s>|<uname -m>|<expected triple>"
SUPPORTED_CASES="
Darwin|arm64|aarch64-apple-darwin
Darwin|aarch64|aarch64-apple-darwin
Darwin|x86_64|x86_64-apple-darwin
Darwin|amd64|x86_64-apple-darwin
Linux|x86_64|x86_64-unknown-linux-gnu
Linux|amd64|x86_64-unknown-linux-gnu
Linux|aarch64|aarch64-unknown-linux-gnu
Linux|arm64|aarch64-unknown-linux-gnu
"

while IFS='|' read -r os arch expected; do
  [ -z "$os" ] && continue
  got="$(run_detect "$os" "$arch")"
  if [ "$got" = "$expected" ]; then
    echo "PASS: $os/$arch → $got"
  else
    echo "FAIL: $os/$arch → '$got' (expected '$expected')"
    ERRORS=$((ERRORS + 1))
  fi
done <<EOF
$SUPPORTED_CASES
EOF

# Cross-check: the set of distinct expected triples must be exactly the four we
# ship. Guards against someone adding a case here without updating the matrix.
EXPECTED_TRIPLES="$(printf '%s\n' "$SUPPORTED_CASES" | grep -v '^$' | cut -d'|' -f3 | sort -u)"
WANT_TRIPLES="$(printf '%s\n' \
  aarch64-apple-darwin \
  x86_64-apple-darwin \
  x86_64-unknown-linux-gnu \
  aarch64-unknown-linux-gnu | sort -u)"
if [ "$EXPECTED_TRIPLES" = "$WANT_TRIPLES" ]; then
  echo "PASS: shipped triple set is exactly the four supported triples"
else
  echo "FAIL: distinct mapped triples do not match the shipped set"
  echo "  got:"
  printf '    %s\n' $EXPECTED_TRIPLES
  echo "  want:"
  printf '    %s\n' $WANT_TRIPLES
  ERRORS=$((ERRORS + 1))
fi

# ----------------------------------------------------------------------------
# (b) Unsupported pairs → empty string (graceful fallback, no triple).
# ----------------------------------------------------------------------------
echo ""
echo "=== Checking unsupported uname → empty (fallback) ==="

# Format: "<uname -s>|<uname -m>|<why>"
UNSUPPORTED_CASES="
Linux|i686|32-bit x86 not shipped
Linux|armv7l|32-bit arm not shipped
Darwin|ppc|legacy arch not shipped
Windows_NT|x86_64|windows not shipped
FreeBSD|x86_64|non-linux/darwin OS not shipped
unknown|unknown|fully unknown host
SunOS|sparc|exotic os+arch
"

while IFS='|' read -r os arch why; do
  [ -z "$os" ] && continue
  got="$(run_detect "$os" "$arch")"
  if [ -z "$got" ]; then
    echo "PASS: $os/$arch → (empty) [$why]"
  else
    echo "FAIL: $os/$arch → '$got' but expected empty [$why]"
    ERRORS=$((ERRORS + 1))
  fi
done <<EOF
$UNSUPPORTED_CASES
EOF

# ----------------------------------------------------------------------------
# (c) Missing-binary path: select_native_tool() must NO-OP (write no stub) when
#     the source bin/ tree holds no binary for the host triple, leaving the
#     vault to fall back to the pure-shell merge.
#
#     We build a throwaway sandbox: a fake SCRIPT_DIR/bin that exists but is
#     EMPTY (no <triple>/xavier-tool), and a fake XAVIER_HOME. After running
#     select_native_tool, XAVIER_HOME/bin must contain no xavier-tool anywhere.
# ----------------------------------------------------------------------------
echo ""
echo "=== Checking select_native_tool no-stub fallback (missing binary) ==="

SELECT_ERRORS=0
SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/xavier-triples.XXXXXX")"
# Best-effort cleanup; never use rm -rf on anything outside the mktemp sandbox.
cleanup() { [ -n "${SANDBOX:-}" ] && [ -d "$SANDBOX" ] && rm -rf "$SANDBOX"; }
trap cleanup EXIT

run_select() {
  # Args: <SCRIPT_DIR> <XAVIER_HOME> <uname -s> <uname -m> <INSTALL_MODE>
  _script_dir="$1"; _xhome="$2"; _os="$3"; _arch="$4"; _mode="$5"
  (
    set +e
    SCRIPT_DIR="$_script_dir"
    XAVIER_HOME="$_xhome"
    INSTALL_MODE="$_mode"
    # Quiet the info/warn helpers select_native_tool calls.
    info() { :; }
    warn() { :; }
    error() { :; }
    uname() {
      case "${1:-}" in
        -s) printf '%s\n' "$_os" ;;
        -m) printf '%s\n' "$_arch" ;;
        *)  printf '%s\n' "unknown" ;;
      esac
    }
    eval "$DETECT_FN"
    eval "$SELECT_FN"
    select_native_tool
  )
}

# Scenario c1: bin/ exists but is empty → no-op for a SUPPORTED triple.
SC1_SRC="$SANDBOX/c1/src"
SC1_HOME="$SANDBOX/c1/home"
mkdir -p "$SC1_SRC/bin" "$SC1_HOME"
run_select "$SC1_SRC" "$SC1_HOME" "Darwin" "arm64" "tarball"
if find "$SC1_HOME" -name xavier-tool 2>/dev/null | grep -q .; then
  echo "FAIL: empty-bin supported-triple case wrote a stub under $SC1_HOME"
  find "$SC1_HOME" -name xavier-tool
  SELECT_ERRORS=$((SELECT_ERRORS + 1))
else
  echo "PASS: empty bin/ + supported triple → no stub (shell fallback)"
fi

# Scenario c2: bin/ exists but is empty → no-op for an UNSUPPORTED triple too.
SC2_SRC="$SANDBOX/c2/src"
SC2_HOME="$SANDBOX/c2/home"
mkdir -p "$SC2_SRC/bin" "$SC2_HOME"
run_select "$SC2_SRC" "$SC2_HOME" "Windows_NT" "x86_64" "tarball"
if find "$SC2_HOME" -name xavier-tool 2>/dev/null | grep -q .; then
  echo "FAIL: empty-bin unsupported-triple case wrote a stub under $SC2_HOME"
  SELECT_ERRORS=$((SELECT_ERRORS + 1))
else
  echo "PASS: empty bin/ + unsupported triple → no stub (shell fallback)"
fi

# Scenario c3: no bin/ dir at all → no-op (clone checkout with no prebuilt bins).
SC3_SRC="$SANDBOX/c3/src"
SC3_HOME="$SANDBOX/c3/home"
mkdir -p "$SC3_SRC" "$SC3_HOME"   # deliberately NO bin/ subdir
run_select "$SC3_SRC" "$SC3_HOME" "Linux" "x86_64" "clone"
if find "$SC3_HOME" -name xavier-tool 2>/dev/null | grep -q .; then
  echo "FAIL: no-bin-dir case wrote a stub under $SC3_HOME"
  SELECT_ERRORS=$((SELECT_ERRORS + 1))
else
  echo "PASS: missing bin/ dir → no stub (shell fallback)"
fi

# Positive control: when a matching binary IS present, select_native_tool must
# install it (proving the no-op cases above are real fallbacks, not a function
# that never does anything). Uses a supported triple for the stubbed host.
SC4_SRC="$SANDBOX/c4/src"
SC4_HOME="$SANDBOX/c4/home"
mkdir -p "$SC4_SRC/bin/x86_64-unknown-linux-gnu" "$SC4_HOME"
printf '#!/bin/sh\necho stub\n' > "$SC4_SRC/bin/x86_64-unknown-linux-gnu/xavier-tool"
run_select "$SC4_SRC" "$SC4_HOME" "Linux" "x86_64" "tarball"
if [ -f "$SC4_HOME/bin/x86_64-unknown-linux-gnu/xavier-tool" ]; then
  echo "PASS: matching binary present → installed into vault (control)"
else
  echo "FAIL: matching binary present but was NOT installed (control)"
  SELECT_ERRORS=$((SELECT_ERRORS + 1))
fi

if [ $SELECT_ERRORS -gt 0 ]; then
  ERRORS=$((ERRORS + SELECT_ERRORS))
fi

# ----------------------------------------------------------------------------
# (d) Kill switch: XAVIER_TOOL_DISABLE must force merge.sh's resolve_tool() to
#     return empty — routing Main to the parse.sh fallback — EVEN when a healthy
#     binary is installed for the host triple. It is the operational rollback,
#     so it has to override a present binary, not merely return empty when none
#     exists.
# ----------------------------------------------------------------------------
echo ""
echo "=== Checking XAVIER_TOOL_DISABLE kill switch (merge.sh resolve_tool) ==="

run_resolve() {
  # Args: <XAVIER_HOME> <uname -s> <uname -m>. Inherits XAVIER_TOOL_DISABLE from
  # the caller's environment so each case can toggle it.
  _xhome="$1"; _os="$2"; _arch="$3"
  (
    set +e
    unset XAVIER_TOOL
    XAVIER_HOME="$_xhome"
    uname() {
      case "${1:-}" in
        -s) printf '%s\n' "$_os" ;;
        -m) printf '%s\n' "$_arch" ;;
        *)  printf '%s\n' "unknown" ;;
      esac
    }
    eval "$RESOLVE_FN"
    resolve_tool
  )
}

# A sandbox vault that DOES have a matching binary for the stubbed host triple.
KS_HOME="$SANDBOX/ks/home"
mkdir -p "$KS_HOME/bin/x86_64-unknown-linux-gnu"
printf '#!/bin/sh\necho stub\n' > "$KS_HOME/bin/x86_64-unknown-linux-gnu/xavier-tool"
chmod +x "$KS_HOME/bin/x86_64-unknown-linux-gnu/xavier-tool"

# Control: not disabled → resolve_tool finds the installed binary.
if [ -n "$(XAVIER_TOOL_DISABLE='' run_resolve "$KS_HOME" Linux x86_64)" ]; then
  echo "PASS: control — resolve_tool finds the installed binary when not disabled"
else
  echo "FAIL: control — resolve_tool found nothing despite a matching installed binary"
  ERRORS=$((ERRORS + 1))
fi

# Kill switch: disabled → resolve_tool returns empty despite the present binary.
if [ -z "$(XAVIER_TOOL_DISABLE=1 run_resolve "$KS_HOME" Linux x86_64)" ]; then
  echo "PASS: XAVIER_TOOL_DISABLE=1 forces empty resolve (shell fallback) over a present binary"
else
  echo "FAIL: XAVIER_TOOL_DISABLE=1 did not disable resolve_tool"
  ERRORS=$((ERRORS + 1))
fi

# ----------------------------------------------------------------------------
# (e) link_or_replace(): clone-mode linker must REPLACE a real-directory
#     destination with a symlink (moving the stale dir aside to .prev), not nest
#     the link inside it. This is the tarball→clone refresh bug: on a
#     tarball-installed vault the skill/reference dests are real dirs, and
#     `ln -sfn <src> <real-dir>` silently created <real-dir>/<basename> and left
#     the stale copy in place while the installer logged success.
#
#     Evaluates the REAL link_or_replace() from install.sh in a subshell with
#     the info/warn/error helpers quieted, so the test tracks the shipped source.
# ----------------------------------------------------------------------------
echo ""
echo "=== Checking link_or_replace replaces a real-dir dest with a symlink ==="

LINK_ERRORS=0

run_link() {
  # Args: <src> <dest>. Runs the extracted link_or_replace against the sandbox.
  _src="$1"; _dest="$2"
  (
    set +e
    info()  { :; }
    warn()  { :; }
    error() { :; }
    eval "$LINK_FN"
    link_or_replace "$_src" "$_dest"
  )
}

# Fake repo (SCRIPT_DIR) skill carries the NEW body; fake vault (XAVIER_HOME)
# skill is a REAL directory holding a STALE body — the tarball-installed shape.
LNK_SRC="$SANDBOX/link/src"
LNK_HOME="$SANDBOX/link/home"
mkdir -p "$LNK_SRC/skills/review" "$LNK_HOME/skills/review"
printf 'NEW-BODY-marker\n'   > "$LNK_SRC/skills/review/SKILL.md"
printf 'STALE-BODY-marker\n' > "$LNK_HOME/skills/review/SKILL.md"

# Guard the call: this script runs under `set -e`, and run_link ends in a
# subshell whose non-zero exit would otherwise abort the whole validator before
# the assertions and LINK_ERRORS tally below could report. Recording the failure
# here keeps the run going and surfaces it in the aggregated count.
if ! run_link "$LNK_SRC/skills/review" "$LNK_HOME/skills/review"; then
  echo "FAIL: link_or_replace exited non-zero on the real-dir scenario"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi

# (1) dest is now a symlink — under the bug it stayed a real dir.
if [ -L "$LNK_HOME/skills/review" ]; then
  echo "PASS: real-dir dest became a symlink"
else
  echo "FAIL: dest is not a symlink (link nested inside the stale dir?)"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi

# (2) no nested self-link — the bug created dest/<basename src> -> src.
if [ -e "$LNK_HOME/skills/review/review" ]; then
  echo "FAIL: nested self-link created inside dest (the bug)"
  LINK_ERRORS=$((LINK_ERRORS + 1))
else
  echo "PASS: no nested self-link inside dest"
fi

# (3) stale copy preserved (reversibly) as a real .prev directory.
if [ -d "$LNK_HOME/skills/review.prev" ] && [ ! -L "$LNK_HOME/skills/review.prev" ]; then
  echo "PASS: stale dir moved aside to review.prev"
else
  echo "FAIL: review.prev is missing or not a real directory"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi

# (4) the swap is real: dest resolves to the FRESH body, .prev holds the STALE one.
if grep -q NEW-BODY-marker "$LNK_HOME/skills/review/SKILL.md" 2>/dev/null; then
  echo "PASS: dest now resolves to the fresh repo body"
else
  echo "FAIL: dest does not resolve to the fresh body"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi
if grep -q STALE-BODY-marker "$LNK_HOME/skills/review.prev/SKILL.md" 2>/dev/null; then
  echo "PASS: stale body preserved under review.prev"
else
  echo "FAIL: stale body not preserved under review.prev"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi

# (5) idempotence: a second run finds dest is a symlink, so the mv-aside guard is
#     skipped and ln -sfn just re-points — no redundant backup. (If the guard
#     wrongly fired on a symlink it would try to mv onto the existing review.prev
#     and exit 1 — caught by the guarded call above, which records it in
#     LINK_ERRORS instead of aborting.)
echo ""
echo "=== Checking link_or_replace is idempotent on a second run ==="
if ! run_link "$LNK_SRC/skills/review" "$LNK_HOME/skills/review"; then
  echo "FAIL: link_or_replace exited non-zero on the idempotent re-run"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi
if [ -L "$LNK_HOME/skills/review" ]; then
  echo "PASS: dest remains a symlink after a second run"
else
  echo "FAIL: second run disturbed the symlink"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi
if [ ! -e "$LNK_HOME/skills/review.prev.prev" ]; then
  echo "PASS: second run created no extra backup (review.prev.prev absent)"
else
  echo "FAIL: second run created a redundant review.prev.prev"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi
if grep -q STALE-BODY-marker "$LNK_HOME/skills/review.prev/SKILL.md" 2>/dev/null; then
  echo "PASS: review.prev still holds the original stale body after re-run"
else
  echo "FAIL: review.prev body changed on the second run"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi

# (6) healthy clone vault: dest is ALREADY a symlink (references shape). The
#     guard must not fire — repoint in place, create NO .prev at all.
echo ""
echo "=== Checking link_or_replace repoints an existing symlink with no backup ==="
REF_SRC_OLD="$SANDBOX/link/refs_old"
REF_SRC_NEW="$SANDBOX/link/refs_new"
REF_HOME="$SANDBOX/link/refhome"
mkdir -p "$REF_SRC_OLD" "$REF_SRC_NEW" "$REF_HOME"
printf 'old\n' > "$REF_SRC_OLD/marker"
printf 'new\n' > "$REF_SRC_NEW/marker"
ln -sfn "$REF_SRC_OLD" "$REF_HOME/references"   # pre-existing healthy symlink

if ! run_link "$REF_SRC_NEW" "$REF_HOME/references"; then
  echo "FAIL: link_or_replace exited non-zero on the existing-symlink scenario"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi

if [ -L "$REF_HOME/references" ] && grep -q '^new$' "$REF_HOME/references/marker" 2>/dev/null; then
  echo "PASS: existing symlink repointed to the new target"
else
  echo "FAIL: existing symlink was not repointed correctly"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi
if [ ! -e "$REF_HOME/references.prev" ]; then
  echo "PASS: no .prev created when dest was already a symlink"
else
  echo "FAIL: spurious references.prev created for a symlink dest"
  LINK_ERRORS=$((LINK_ERRORS + 1))
fi

if [ $LINK_ERRORS -gt 0 ]; then
  ERRORS=$((ERRORS + LINK_ERRORS))
fi

echo ""
if [ $ERRORS -gt 0 ]; then
  echo "INSTALL TRIPLES: $ERRORS error(s) found"
  exit 1
else
  echo "INSTALL TRIPLES: ALL CHECKS PASSED"
  exit 0
fi
