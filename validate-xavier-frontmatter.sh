#!/bin/bash
set -euo pipefail

# Validate frontmatter in all xavier sub-skills
# Checks:
# 1. Every xavier/skills/*/SKILL.md has valid frontmatter with 'name' and 'requires'
# 2. The 'name' field matches the directory name
# 3. Every entry in 'requires' is in the 13-key vocabulary

ERRORS=0
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILLS_DIR="$REPO_ROOT/xavier/skills"

# The 13-key vocabulary
VALID_REQUIRES="config personas shark adapter recurring-patterns team-conventions repo-conventions prd-index tasks-index skills-index deps-index vault-memory"

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
    # Strip :required or :optional annotation before validating key name
    req_base="$(echo "$req" | sed 's/:required$//;s/:optional$//')"
    # Validate the annotation itself (only :required or :optional allowed)
    req_annotation="$(echo "$req" | grep -o ':[a-z]*$' || true)"
    if [ -n "$req_annotation" ] && [ "$req_annotation" != ":required" ] && [ "$req_annotation" != ":optional" ]; then
      echo "FAIL: $dir_name/SKILL.md has invalid requires annotation: '$req' (only :required or :optional allowed)"
      ERRORS=$((ERRORS + 1))
      req_valid=false
    fi
    if ! echo "$VALID_REQUIRES" | grep -qw "$req_base"; then
      echo "FAIL: $dir_name/SKILL.md has invalid requires key: '$req_base'"
      ERRORS=$((ERRORS + 1))
      req_valid=false
    fi
  done <<< "$requires_clean"

  if $req_valid; then
    echo "PASS: $dir_name (requires)"
  fi
done

# Check that note-writing skills include all 6 base Zettelkasten fields in their templates
echo ""
echo "=== Checking base Zettelkasten fields in note-writing skill templates ==="
NOTE_WRITING_SKILLS="learn review prd tasks"
BASE_FIELDS="repo type created updated tags related"

for skill_name in $NOTE_WRITING_SKILLS; do
  skill_file="$SKILLS_DIR/$skill_name/SKILL.md"
  [ -f "$skill_file" ] || continue

  # Extract YAML template blocks from the skill body (after the skill's own frontmatter).
  # These are code blocks that start with ```yaml or ``` followed by --- on the next line.
  # We look for YAML blocks containing "type:" which indicates a note template (not a bash block).
  # We skip the skill's own frontmatter (the very first --- ... --- block at the top of the file).
  body="$(awk 'BEGIN{c=0} /^---$/{c++; if(c==2){getline; found=1}} found{print}' "$skill_file")"

  # Extract all YAML/markdown code blocks from the body that contain "type:" (note templates)
  # Opening: ```yaml or ```markdown or ```  Closing: ```
  templates="$(echo "$body" | awk '
    /^```(yaml|markdown)/ && !in_block { in_block=1; block=""; next }
    /^```$/ && in_block { if (block ~ /type:/) print block; in_block=0; next }
    in_block { block = block "\n" $0 }
  ')"

  if [ -z "$templates" ]; then
    echo "WARN: $skill_name has no YAML note templates found"
    continue
  fi

  skill_ok=true
  for field in $BASE_FIELDS; do
    if ! echo "$templates" | grep -q "^${field}:"; then
      echo "FAIL: $skill_name/SKILL.md template is missing base field '$field'"
      ERRORS=$((ERRORS + 1))
      skill_ok=false
    fi
  done

  if $skill_ok; then
    echo "PASS: $skill_name (base Zettelkasten fields)"
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
