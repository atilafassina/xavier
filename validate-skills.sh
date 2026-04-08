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

echo ""
if [ $ERRORS -gt 0 ]; then
  echo "FAILED: $ERRORS error(s) found"
  exit 1
else
  echo "ALL CHECKS PASSED"
  exit 0
fi
