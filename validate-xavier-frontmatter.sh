#!/bin/bash
set -euo pipefail

# Validate frontmatter in all xavier sub-skills
# Checks:
# 1. Every xavier/skills/*/SKILL.md has valid frontmatter with 'name' and 'requires'
# 2. The 'name' field matches the directory name
# 3. Every entry in 'requires' is in the 14-key vocabulary

ERRORS=0
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILLS_DIR="$REPO_ROOT/xavier/skills"

# The 14-key vocabulary
VALID_REQUIRES="config personas shark adapter recurring-patterns team-conventions repo-conventions prd-index tasks-index skills-index deps-index vault-memory research-index investigations-index"

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

# Validate `status` field in vault notes (when present, must be `done` or `superseded`).
# Walks XAVIER_HOME's prd/ and tasks/ trees if it exists (the only directories that own
# the lifecycle field), plus any vault note fixtures passed via XAVIER_VALIDATE_PATHS
# (newline-delimited). Notes without a `status` field pass silently.
echo ""
echo "=== Checking optional 'status' field in vault notes ==="
STATUS_ERRORS=0

# Build search roots as a newline-delimited list. XAVIER_HOME contributes its prd/ and
# tasks/ subtrees only — the rest of the vault is out of scope for this check.
SEARCH_ROOTS=""
add_root() {
  # Reject entries that find(1) could parse as a primary expression.
  case "$1" in
    -*|!*|"("*|")"*) echo "FAIL: refusing unsafe path '$1' (must not start with -, !, or parens)" >&2; STATUS_ERRORS=$((STATUS_ERRORS + 1)); return ;;
  esac
  [ -e "$1" ] || return
  if [ -z "$SEARCH_ROOTS" ]; then SEARCH_ROOTS="$1"; else SEARCH_ROOTS="$SEARCH_ROOTS
$1"; fi
}

if [ -n "${XAVIER_HOME:-}" ] && [ -d "${XAVIER_HOME}" ]; then
  [ -d "${XAVIER_HOME}/prd" ] && add_root "${XAVIER_HOME}/prd"
  [ -d "${XAVIER_HOME}/tasks" ] && add_root "${XAVIER_HOME}/tasks"
fi
if [ -n "${XAVIER_VALIDATE_PATHS:-}" ]; then
  # Newline-delimited only — no whitespace-splitting to dodge argument-injection footguns.
  while IFS= read -r extra_root; do
    [ -z "$extra_root" ] && continue
    add_root "$extra_root"
  done <<EOF
${XAVIER_VALIDATE_PATHS}
EOF
fi

if [ -n "$SEARCH_ROOTS" ]; then
  while IFS= read -r root; do
    [ -e "$root" ] || continue

    # Collect candidate files. For directories we want every .md file (so we can also
    # detect done/-side notes that are missing the mandatory status field), not just
    # files that already grep positive for `status:`.
    if [ -d "$root" ]; then
      # `--` keeps find treating $root as a path even if a future caller sneaks a
      # leading hyphen past the add_root vetting.
      candidates="$(find -- "$root" -type f -name '*.md' 2>/dev/null || true)"
    else
      candidates="$root"
    fi

    [ -z "$candidates" ] && continue

    while IFS= read -r note_file; do
      [ -f "$note_file" ] || continue

      # Determine whether this file lives in a done/ subtree — done/ files MUST carry
      # a valid status (per references/formats/zettelkasten.md canonical state rules).
      is_done_side=false
      case "$note_file" in
        */prd/done/*|*/tasks/done/*) is_done_side=true ;;
      esac

      # Extract status field from the first frontmatter block only.
      status_value="$(awk '/^---$/{c++; if(c==2) exit; next} c==1 && /^status:/{sub(/^status:[[:space:]]*/, ""); gsub(/[[:space:]]*$/, ""); print; exit}' "$note_file")"

      # No status field on a top-level file → accept silently (canonical active).
      # No status field on a done/-side file → FAIL (canonical state requires it).
      if [ -z "$status_value" ]; then
        if [ "$is_done_side" = "true" ]; then
          printf 'FAIL: %s lives in done/ but is missing the mandatory status field\n' "$note_file"
          STATUS_ERRORS=$((STATUS_ERRORS + 1))
        fi
        continue
      fi

      # Strip surrounding quotes if present (parameter expansion — no subshell, no echo).
      status_value="${status_value#[\"\']}"
      status_value="${status_value%[\"\']}"

      # Exact string match — never delegate to grep (which would let `.*` bypass the allowlist).
      # printf instead of echo so leading -e / -n etc. cannot be parsed as options.
      if [ "$status_value" != "done" ] && [ "$status_value" != "superseded" ]; then
        printf 'FAIL: %s has invalid status: %s (allowed: done, superseded)\n' "$note_file" "'$status_value'"
        STATUS_ERRORS=$((STATUS_ERRORS + 1))
      fi
    done <<< "$candidates"
  done <<EOF
${SEARCH_ROOTS}
EOF
fi

if [ $STATUS_ERRORS -eq 0 ]; then
  echo "PASS: 'status' field validation"
else
  ERRORS=$((ERRORS + STATUS_ERRORS))
fi

# Check that note-writing skills include all 6 base Zettelkasten fields in their templates
echo ""
echo "=== Checking base Zettelkasten fields in note-writing skill templates ==="
NOTE_WRITING_SKILLS="learn review prd tasks research investigate"
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
