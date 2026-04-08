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

echo ""
if [ $ERRORS -gt 0 ]; then
  echo "FAILED: $ERRORS error(s) found"
  exit 1
else
  echo "ALL CHECKS PASSED"
  exit 0
fi
