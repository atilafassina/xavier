#!/bin/bash
set -euo pipefail

ERRORS=0
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 1. Check for stale plans/ references in all SKILL.md files
echo "=== Checking for stale plans/ references ==="
while IFS= read -r skill_file; do
  if grep -q 'plans/' "$skill_file"; then
    echo "FAIL: $skill_file still references plans/"
    grep -n 'plans/' "$skill_file"
    ERRORS=$((ERRORS + 1))
  fi
done < <(find "$REPO_ROOT" -name "SKILL.md" -not -path "*/node_modules/*")

if [ $ERRORS -eq 0 ]; then
  echo "PASS: No stale plans/ references found"
fi

# 2. Check for raw Agent() calls in SKILL.md files (should use adapter spawn/collect)
echo ""
echo "=== Checking for raw Agent() calls ==="
RAW_AGENT_FOUND=0
while IFS= read -r skill_file; do
  if grep -q 'Agent(' "$skill_file"; then
    echo "FAIL: $skill_file contains raw Agent() call — use adapter spawn()/collect()"
    RAW_AGENT_FOUND=$((RAW_AGENT_FOUND + 1))
  fi
done < <(find "$REPO_ROOT/xavier/skills" -name "SKILL.md" -not -path "*/node_modules/*")

if [ $RAW_AGENT_FOUND -eq 0 ]; then
  echo "PASS: No raw Agent() calls found in skills"
else
  ERRORS=$((ERRORS + RAW_AGENT_FOUND))
fi

# 3. Check frontmatter name matches directory name (xavier sub-skills)
echo ""
echo "=== Checking frontmatter name consistency ==="
for skill_dir in "$REPO_ROOT"/xavier/skills/*/; do
  skill_file="$skill_dir/SKILL.md"
  [ -f "$skill_file" ] || continue

  dir_name="$(basename "$skill_dir")"
  fm_name="$(awk '/^---$/{c++; next} c==1 && /^name:/{sub(/^name: */, ""); print; exit}' "$skill_file")"

  if [ -z "$fm_name" ]; then
    echo "WARN: $dir_name/SKILL.md has no frontmatter name field"
  elif [ "$fm_name" != "$dir_name" ]; then
    echo "FAIL: Directory '$dir_name' does not match frontmatter name '$fm_name'"
    ERRORS=$((ERRORS + 1))
  else
    echo "PASS: $dir_name"
  fi
done

# 4. Validate xavier sub-skill frontmatter
echo ""
echo "=== Checking xavier sub-skill frontmatter ==="
if [ -x "$REPO_ROOT/validate-xavier-frontmatter.sh" ]; then
  if ! "$REPO_ROOT/validate-xavier-frontmatter.sh"; then
    ERRORS=$((ERRORS + 1))
  fi
else
  echo "WARN: validate-xavier-frontmatter.sh not found or not executable"
fi

# 5. Check for undeclared vault-path reads in skills
echo ""
echo "=== Checking for undeclared vault-path reads ==="
VAULT_PATH_ERRORS=0

# Map vault path patterns to required keys
# Format: "pattern|requires_key|description"
# Skills that only WRITE to a path don't need the requires key.
# We check the skill body (after frontmatter) for path patterns that imply READING.
check_vault_path() {
  local skill_file="$1"
  local skill_name="$2"

  # Extract requires list from frontmatter
  local requires_line
  requires_line="$(awk '/^---$/{c++; next} c==1 && /^requires:/{sub(/^requires: */, ""); print; exit}' "$skill_file")"
  local requires_clean
  requires_clean="$(echo "$requires_line" | tr -d '[]' | tr ',' '\n' | sed 's/^ *//;s/ *$//' | sed 's/:required$//;s/:optional$//' | grep -v '^$' || true)"

  # Extract skill body (after frontmatter)
  local body
  body="$(awk 'BEGIN{c=0} /^---$/{c++; if(c==2) {getline; found=1}} found{print}' "$skill_file")"

  # Define read-indicating patterns and their required keys
  # We look for patterns that suggest READING from vault paths
  # Patterns: "list.*prd/" or "read.*prd/" or "from.*prd/" suggest reading
  # Patterns like "write.*prd/" or "to.*prd/" suggest writing (OK without requires)

  # Check prd/ reads -> needs prd-index
  if echo "$body" | grep -qiE '(list|read|select|browse|from|in|show|present|scan|check|load).*[~/.<].*prd/' 2>/dev/null; then
    if ! echo "$requires_clean" | grep -qw "prd-index" 2>/dev/null; then
      echo "FAIL: $skill_name reads from prd/ but does not declare 'prd-index' in requires"
      VAULT_PATH_ERRORS=$((VAULT_PATH_ERRORS + 1))
    fi
  fi

  # Check tasks/ reads -> needs tasks-index
  if echo "$body" | grep -qiE '(list|read|select|browse|from|in|show|present|scan|check|load|pick|accept|available).*[~/.<].*tasks/' 2>/dev/null; then
    if ! echo "$requires_clean" | grep -qw "tasks-index" 2>/dev/null; then
      echo "FAIL: $skill_name reads from tasks/ but does not declare 'tasks-index' in requires"
      VAULT_PATH_ERRORS=$((VAULT_PATH_ERRORS + 1))
    fi
  fi

  # Check knowledge/repos/ reads -> needs repo-conventions
  if echo "$body" | grep -qiE '(list|read|select|browse|from|in|show|present|scan|check|load).*[~/.<].*knowledge/repos/' 2>/dev/null; then
    if ! echo "$requires_clean" | grep -qw "repo-conventions" 2>/dev/null; then
      echo "FAIL: $skill_name reads from knowledge/repos/ but does not declare 'repo-conventions' in requires"
      VAULT_PATH_ERRORS=$((VAULT_PATH_ERRORS + 1))
    fi
  fi

  # Check knowledge/teams/ reads -> needs team-conventions
  if echo "$body" | grep -qiE '(list|read|select|browse|from|in|show|present|scan|check|load).*[~/.<].*knowledge/teams/' 2>/dev/null; then
    if ! echo "$requires_clean" | grep -qw "team-conventions" 2>/dev/null; then
      echo "FAIL: $skill_name reads from knowledge/teams/ but does not declare 'team-conventions' in requires"
      VAULT_PATH_ERRORS=$((VAULT_PATH_ERRORS + 1))
    fi
  fi

  # Check knowledge/reviews/ reads -> needs recurring-patterns
  if echo "$body" | grep -qiE '(list|read|select|browse|from|in|show|present|scan|check|load|extract|recent).*[~/.<].*knowledge/reviews/' 2>/dev/null; then
    if ! echo "$requires_clean" | grep -qw "recurring-patterns" 2>/dev/null; then
      echo "FAIL: $skill_name reads from knowledge/reviews/ but does not declare 'recurring-patterns' in requires"
      VAULT_PATH_ERRORS=$((VAULT_PATH_ERRORS + 1))
    fi
  fi

  # Check references/personas/ reads -> needs personas
  if echo "$body" | grep -qiE '(list|read|load|from).*[~/.<].*references/personas/' 2>/dev/null; then
    if ! echo "$requires_clean" | grep -qw "personas" 2>/dev/null; then
      echo "FAIL: $skill_name reads from references/personas/ but does not declare 'personas' in requires"
      VAULT_PATH_ERRORS=$((VAULT_PATH_ERRORS + 1))
    fi
  fi

  # Check deps/ reads -> needs deps-index
  if echo "$body" | grep -qiE '(list|read|select|browse|from|in|show|present|scan|check|compare).*[~/.<].*deps/' 2>/dev/null; then
    if ! echo "$requires_clean" | grep -qw "deps-index" 2>/dev/null; then
      echo "FAIL: $skill_name reads from deps/ but does not declare 'deps-index' in requires"
      VAULT_PATH_ERRORS=$((VAULT_PATH_ERRORS + 1))
    fi
  fi

  # Check research/ reads -> needs research-index
  if echo "$body" | grep -qiE '(list|read|select|browse|from|in|show|present|scan|check|load|glob).*[~/.<].*research/' 2>/dev/null; then
    if ! echo "$requires_clean" | grep -qw "research-index" 2>/dev/null; then
      echo "FAIL: $skill_name reads from research/ but does not declare 'research-index' in requires"
      VAULT_PATH_ERRORS=$((VAULT_PATH_ERRORS + 1))
    fi
  fi

  # Check knowledge/qa/ reads -> needs qa-index
  if echo "$body" | grep -qiE '(list|read|select|browse|from|in|show|present|scan|check|load|glob).*[~/.<].*knowledge/qa/' 2>/dev/null; then
    if ! echo "$requires_clean" | grep -qw "qa-index" 2>/dev/null; then
      echo "FAIL: $skill_name reads from knowledge/qa/ but does not declare 'qa-index' in requires"
      VAULT_PATH_ERRORS=$((VAULT_PATH_ERRORS + 1))
    fi
  fi
}

for skill_dir in "$REPO_ROOT"/xavier/skills/*/; do
  skill_file="$skill_dir/SKILL.md"
  [ -f "$skill_file" ] || continue
  skill_name="$(basename "$skill_dir")"
  check_vault_path "$skill_file" "$skill_name"
done

if [ $VAULT_PATH_ERRORS -eq 0 ]; then
  echo "PASS: No undeclared vault-path reads found"
else
  ERRORS=$((ERRORS + VAULT_PATH_ERRORS))
fi

# 6. Validate adapter files
echo ""
echo "=== Checking adapter files ==="
ADAPTER_ERRORS=0

for adapter_dir in "$REPO_ROOT"/xavier/references/adapters/*/; do
  [ -d "$adapter_dir" ] || continue
  adapter_name="$(basename "$adapter_dir")"
  adapter_file="$adapter_dir/adapter.md"
  adapter_errors_local=0

  if [ ! -f "$adapter_file" ]; then
    echo "FAIL: Adapter directory '$adapter_name' has no adapter.md"
    adapter_errors_local=$((adapter_errors_local + 1))
    ADAPTER_ERRORS=$((ADAPTER_ERRORS + adapter_errors_local))
    continue
  fi

  # Check required frontmatter keys: name, type, runtime
  for key in name type runtime; do
    if ! awk '/^---$/{c++; next} c==1{print}' "$adapter_file" | grep -q "^${key}:"; then
      echo "FAIL: $adapter_name/adapter.md missing frontmatter key '$key'"
      adapter_errors_local=$((adapter_errors_local + 1))
    fi
  done

  # Check required method sections: spawn, collect, poll
  for method in "spawn" "collect" "poll"; do
    if ! grep -qi "## ${method}" "$adapter_file"; then
      echo "FAIL: $adapter_name/adapter.md missing '## ${method}' section"
      adapter_errors_local=$((adapter_errors_local + 1))
    fi
  done

  # Check for Tool Dispatch table
  if ! grep -qi "Tool Dispatch" "$adapter_file"; then
    echo "FAIL: $adapter_name/adapter.md missing 'Tool Dispatch' section"
    adapter_errors_local=$((adapter_errors_local + 1))
  fi

  ADAPTER_ERRORS=$((ADAPTER_ERRORS + adapter_errors_local))
  if [ $adapter_errors_local -eq 0 ]; then
    echo "PASS: $adapter_name"
  fi
done

if [ $ADAPTER_ERRORS -eq 0 ]; then
  echo "PASS: All adapter files valid"
else
  ERRORS=$((ERRORS + ADAPTER_ERRORS))
fi

# 7. Marker-drift check: prose-trigger BEGIN/END markers must be byte-identical
#    between xavier/install.sh and xavier/skills/self-update/SKILL.md.
#    Both files write the same managed block to ~/.claude/CLAUDE.md; if either
#    drifts, install would write a block that self-update can no longer find
#    and replace (leading to duplicate blocks on the next self-update run).
echo ""
echo "=== Checking prose-trigger marker consistency ==="
MARKER_ERRORS=0
EXPECTED_BEGIN='<!-- BEGIN xavier-prose-trigger -->'
EXPECTED_END='<!-- END xavier-prose-trigger -->'
INSTALL_SH="$REPO_ROOT/xavier/install.sh"
SELF_UPDATE_MD="$REPO_ROOT/xavier/skills/self-update/SKILL.md"

for target in "$INSTALL_SH" "$SELF_UPDATE_MD"; do
  rel="${target#$REPO_ROOT/}"
  if [ ! -f "$target" ]; then
    echo "FAIL: MARKER DRIFT — expected file not found: $rel"
    MARKER_ERRORS=$((MARKER_ERRORS + 1))
    continue
  fi
  if ! grep -qF "$EXPECTED_BEGIN" "$target"; then
    echo "FAIL: MARKER DRIFT — $rel missing BEGIN marker '$EXPECTED_BEGIN'"
    MARKER_ERRORS=$((MARKER_ERRORS + 1))
  fi
  if ! grep -qF "$EXPECTED_END" "$target"; then
    echo "FAIL: MARKER DRIFT — $rel missing END marker '$EXPECTED_END'"
    MARKER_ERRORS=$((MARKER_ERRORS + 1))
  fi
done

# Cross-file equality: every distinct `BEGIN xavier-prose-trigger` /
# `END xavier-prose-trigger` literal in either file must match the canonical
# form. We grep case-insensitively to surface lowercase or whitespace variants
# (e.g. `<!--BEGIN ...-->`, `<!-- begin ... -->`) that would slip past a strict
# uppercase grep but still break runtime marker-matching between the two writers.
if [ -f "$INSTALL_SH" ] && [ -f "$SELF_UPDATE_MD" ]; then
  install_begin_variants="$(grep -oiE '<!--[[:space:]]*BEGIN[[:space:]]+xavier-prose-trigger[[:space:]]*-->' "$INSTALL_SH" | sort -u)"
  selfupd_begin_variants="$(grep -oiE '<!--[[:space:]]*BEGIN[[:space:]]+xavier-prose-trigger[[:space:]]*-->' "$SELF_UPDATE_MD" | sort -u)"
  install_end_variants="$(grep -oiE '<!--[[:space:]]*END[[:space:]]+xavier-prose-trigger[[:space:]]*-->' "$INSTALL_SH" | sort -u)"
  selfupd_end_variants="$(grep -oiE '<!--[[:space:]]*END[[:space:]]+xavier-prose-trigger[[:space:]]*-->' "$SELF_UPDATE_MD" | sort -u)"

  # Each file must contain ONLY the canonical marker literal — no variants.
  # A file carrying multiple distinct forms means a partial edit drifted the
  # writer-time string away from the comment/doc-time string.
  for pair in "install.sh|$install_begin_variants|$EXPECTED_BEGIN|BEGIN" \
              "self-update SKILL.md|$selfupd_begin_variants|$EXPECTED_BEGIN|BEGIN" \
              "install.sh|$install_end_variants|$EXPECTED_END|END" \
              "self-update SKILL.md|$selfupd_end_variants|$EXPECTED_END|END"; do
    label="${pair%%|*}"
    rest="${pair#*|}"
    variants="${rest%%|*}"
    rest="${rest#*|}"
    expected="${rest%%|*}"
    kind="${rest#*|}"
    # Strip surrounding whitespace and check every distinct variant matches the canonical.
    if [ -n "$variants" ]; then
      printf '%s\n' "$variants" | while IFS= read -r v; do
        [ -z "$v" ] && continue
        if [ "$v" != "$expected" ]; then
          echo "FAIL: MARKER DRIFT — $label has non-canonical $kind marker '$v' (expected '$expected')"
          # Bubble up via a sentinel file since this is inside a subshell.
          touch /tmp/.xavier-marker-drift-$$
        fi
      done
    fi
  done
  if [ -f "/tmp/.xavier-marker-drift-$$" ]; then
    MARKER_ERRORS=$((MARKER_ERRORS + 1))
    rm -f "/tmp/.xavier-marker-drift-$$"
  fi

  # Cross-file equality: the canonical-set of marker literals in each file
  # must be identical between install.sh and self-update SKILL.md.
  if [ "$install_begin_variants" != "$selfupd_begin_variants" ]; then
    echo "FAIL: MARKER DRIFT — BEGIN marker text differs between install.sh and self-update SKILL.md"
    echo "  install.sh: $install_begin_variants"
    echo "  self-update SKILL.md: $selfupd_begin_variants"
    MARKER_ERRORS=$((MARKER_ERRORS + 1))
  fi
  if [ "$install_end_variants" != "$selfupd_end_variants" ]; then
    echo "FAIL: MARKER DRIFT — END marker text differs between install.sh and self-update SKILL.md"
    echo "  install.sh: $install_end_variants"
    echo "  self-update SKILL.md: $selfupd_end_variants"
    MARKER_ERRORS=$((MARKER_ERRORS + 1))
  fi
fi

if [ $MARKER_ERRORS -eq 0 ]; then
  echo "PASS: prose-trigger markers consistent across install.sh and self-update SKILL.md"
else
  ERRORS=$((ERRORS + MARKER_ERRORS))
fi

# 8. Cursor prose-trigger skill template drift — install.sh and self-update
#    must carry the same anchor strings for the Cursor skill writer.
echo ""
echo "=== Checking Cursor prose-trigger skill template consistency ==="
CURSOR_PROSE_ERRORS=0
EXPECTED_SKILL_NAME='name: prose-trigger'
EXPECTED_ROUTER_LINE='Follow the Router Lifecycle with subcommand'
EXPECTED_VOCATIVE_FRAGMENT='Mid-sentence "${TRIGGER_WORD}" or lowercase variants do NOT'

for anchor in "$EXPECTED_SKILL_NAME" "$EXPECTED_ROUTER_LINE" "$EXPECTED_VOCATIVE_FRAGMENT"; do
  for target in "$INSTALL_SH" "$SELF_UPDATE_MD"; do
    rel="${target#$REPO_ROOT/}"
    if [ ! -f "$target" ]; then
      echo "FAIL: CURSOR PROSE DRIFT — expected file not found: $rel"
      CURSOR_PROSE_ERRORS=$((CURSOR_PROSE_ERRORS + 1))
      continue
    fi
    if ! grep -qF "$anchor" "$target"; then
      echo "FAIL: CURSOR PROSE DRIFT — $rel missing anchor '$anchor'"
      CURSOR_PROSE_ERRORS=$((CURSOR_PROSE_ERRORS + 1))
    fi
  done
done

if [ $CURSOR_PROSE_ERRORS -eq 0 ]; then
  echo "PASS: Cursor prose-trigger skill template anchors consistent across install.sh and self-update SKILL.md"
else
  ERRORS=$((ERRORS + CURSOR_PROSE_ERRORS))
fi

echo ""
if [ $ERRORS -gt 0 ]; then
  echo "FAILED: $ERRORS error(s) found"
  exit 1
else
  echo "ALL CHECKS PASSED"
  exit 0
fi
