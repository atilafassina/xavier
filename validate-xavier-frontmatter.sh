#!/bin/bash
set -euo pipefail

# Validate frontmatter in all xavier sub-skills
# Checks:
# 1. Every xavier/skills/*/SKILL.md has valid frontmatter with 'name' and 'requires'
# 2. The 'name' field matches the directory name
# 3. Every entry in 'requires' is in the 12-key vocabulary

ERRORS=0
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILLS_DIR="$REPO_ROOT/skills"

# The 12-key vocabulary
VALID_REQUIRES="config personas shark adapter recurring-patterns team-conventions repo-conventions prd-index tasks-index skills-index vault-memory"

if [ ! -d "$SKILLS_DIR" ]; then
  echo "FAIL: $SKILLS_DIR does not exist"
  exit 1
fi

for skill_dir in "$SKILLS_DIR"/*/; do
  skill_file="$skill_dir/SKILL.md"
  [ -f "$skill_file" ] || continue

  dir_name="$(basename "$skill_dir")"

  # Extract frontmatter name
  fm_name="$(awk '/^---$/{c++; next} c==1 && /^name:/{sub(/^name: */, ""); print; exit}' "$skill_file")"

  if [ -z "$fm_name" ]; then
    echo "FAIL: $dir_name/SKILL.md has no frontmatter 'name' field"
    ERRORS=$((ERRORS + 1))
  elif [ "$fm_name" != "$dir_name" ]; then
    echo "FAIL: Directory '$dir_name' does not match frontmatter name '$fm_name'"
    ERRORS=$((ERRORS + 1))
  else
    echo "PASS: $dir_name (name)"
  fi

  # Extract requires list
  requires_line="$(awk '/^---$/{c++; next} c==1 && /^requires:/{sub(/^requires: */, ""); print; exit}' "$skill_file")"

  if [ -z "$requires_line" ]; then
    echo "FAIL: $dir_name/SKILL.md has no frontmatter 'requires' field"
    ERRORS=$((ERRORS + 1))
    continue
  fi

  # Parse requires: [] or requires: [key1, key2, ...]
  # Strip brackets and split by comma
  requires_clean="$(echo "$requires_line" | tr -d '[]' | tr ',' '\n' | sed 's/^ *//;s/ *$//' | grep -v '^$' || true)"

  if [ -z "$requires_clean" ]; then
    echo "PASS: $dir_name (requires: [] — empty)"
    continue
  fi

  req_valid=true
  while IFS= read -r req; do
    if ! echo "$VALID_REQUIRES" | grep -qw "$req"; then
      echo "FAIL: $dir_name/SKILL.md has invalid requires key: '$req'"
      ERRORS=$((ERRORS + 1))
      req_valid=false
    fi
  done <<< "$requires_clean"

  if $req_valid; then
    echo "PASS: $dir_name (requires)"
  fi
done

echo ""
if [ $ERRORS -gt 0 ]; then
  echo "XAVIER FRONTMATTER: $ERRORS error(s) found"
  exit 1
else
  echo "XAVIER FRONTMATTER: ALL CHECKS PASSED"
  exit 0
fi
