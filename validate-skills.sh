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

# 7. Check runtime alias generation stays in sync for Codex
echo ""
echo "=== Checking Codex alias wiring ==="
CODEX_ALIAS_ERRORS=0

if ! grep -q '\.agents/skills.*\${ALIAS_PREFIX}-\${cmd}' "$REPO_ROOT/xavier/install.sh"; then
  echo "FAIL: xavier/install.sh does not generate Codex per-command aliases"
  CODEX_ALIAS_ERRORS=$((CODEX_ALIAS_ERRORS + 1))
fi

if ! grep -q 'XAVIER_HOME/SKILL.md' "$REPO_ROOT/xavier/install.sh" || ! grep -q 'ln -sfn "$SKILL_SOURCE" "$XAVIER_HOME/SKILL.md"' "$REPO_ROOT/xavier/install.sh"; then
  echo "FAIL: xavier/install.sh clone mode does not refresh the vault router used by Codex aliases"
  CODEX_ALIAS_ERRORS=$((CODEX_ALIAS_ERRORS + 1))
fi

if ! grep -q 'PRESERVE_CONFIG=true' "$REPO_ROOT/xavier/install.sh" || ! grep -q 'Preserving primary adapter' "$REPO_ROOT/xavier/install.sh"; then
  echo "FAIL: xavier/install.sh refresh-only path does not preserve config.md adapter settings"
  CODEX_ALIAS_ERRORS=$((CODEX_ALIAS_ERRORS + 1))
fi

if ! grep -q 'refresh_available_adapters' "$REPO_ROOT/xavier/install.sh"; then
  echo "FAIL: xavier/install.sh does not refresh available-adapters on refresh installs"
  CODEX_ALIAS_ERRORS=$((CODEX_ALIAS_ERRORS + 1))
fi

if ! grep -q 'case " \$DETECTED_RUNTIMES "' "$REPO_ROOT/xavier/install.sh"; then
  echo "FAIL: xavier/install.sh does not gate symlink/alias writes by detected runtimes"
  CODEX_ALIAS_ERRORS=$((CODEX_ALIAS_ERRORS + 1))
fi

if ! grep -q 'case " \$DETECTED_RUNTIMES "' "$REPO_ROOT/xavier/skills/self-update/SKILL.md"; then
  echo "FAIL: self-update SKILL.md does not gate alias regeneration by detected runtimes"
  CODEX_ALIAS_ERRORS=$((CODEX_ALIAS_ERRORS + 1))
fi

if ! grep -q '\.agents/skills.*\${ALIAS_PREFIX}-\${cmd}' "$REPO_ROOT/xavier/skills/self-update/SKILL.md"; then
  echo "FAIL: self-update does not regenerate Codex per-command aliases"
  CODEX_ALIAS_ERRORS=$((CODEX_ALIAS_ERRORS + 1))
fi

if ! grep -q '\.agents/skills"/\${ALIAS_PREFIX}-\*/' "$REPO_ROOT/uninstall.sh"; then
  echo "FAIL: uninstall.sh does not remove Codex per-command aliases"
  CODEX_ALIAS_ERRORS=$((CODEX_ALIAS_ERRORS + 1))
fi

if [ $CODEX_ALIAS_ERRORS -eq 0 ]; then
  echo "PASS: Codex alias wiring present"
else
  ERRORS=$((ERRORS + CODEX_ALIAS_ERRORS))
fi

# 8. Check review-integration: ace stays a hard gate
echo ""
echo "=== Checking review integration gate ==="
REVIEW_GATE_ERRORS=0
REVIEW_SKILL="$REPO_ROOT/xavier/skills/review/SKILL.md"

if ! grep -q 'review-integration' "$REVIEW_SKILL"; then
  echo "FAIL: review skill does not read review-integration from config"
  REVIEW_GATE_ERRORS=$((REVIEW_GATE_ERRORS + 1))
fi

if ! grep -q 'debate_required = true' "$REVIEW_SKILL"; then
  echo "FAIL: review skill does not set debate_required for ace integration"
  REVIEW_GATE_ERRORS=$((REVIEW_GATE_ERRORS + 1))
fi

if ! grep -q 'Do \*\*not\*\* run the standard three-persona flow when `review-integration: ace` is configured' "$REVIEW_SKILL"; then
  echo "FAIL: review skill does not hard-fail instead of falling back when ace debate is unavailable"
  REVIEW_GATE_ERRORS=$((REVIEW_GATE_ERRORS + 1))
fi

if ! grep -q 'This path is only allowed when `debate_required = false`' "$REVIEW_SKILL"; then
  echo "FAIL: review skill does not guard the standard review path behind debate_required=false"
  REVIEW_GATE_ERRORS=$((REVIEW_GATE_ERRORS + 1))
fi

if [ $REVIEW_GATE_ERRORS -eq 0 ]; then
  echo "PASS: review-integration ace hard gate present"
else
  ERRORS=$((ERRORS + REVIEW_GATE_ERRORS))
fi

# 9. Check Codex remora status uses labels, not raw handles
echo ""
echo "=== Checking Codex remora labels ==="
CODEX_LABEL_ERRORS=0
CODEX_ADAPTER="$REPO_ROOT/xavier/references/adapters/codex/adapter.md"
INSTALLER="$REPO_ROOT/xavier/install.sh"

for file in "$CODEX_ADAPTER" "$INSTALLER"; do
  if ! grep -q 'Xavier remora:' "$file"; then
    echo "FAIL: $(basename "$file") does not prefix Codex subagent messages with a remora label"
    CODEX_LABEL_ERRORS=$((CODEX_LABEL_ERRORS + 1))
  fi

  if ! grep -q 'label, nickname, handle' "$file"; then
    echo "FAIL: $(basename "$file") does not require a label/nickname/handle agent map"
    CODEX_LABEL_ERRORS=$((CODEX_LABEL_ERRORS + 1))
  fi

  if ! grep -q 'raw agent hashes' "$file" && ! grep -q 'raw handles' "$file"; then
    echo "FAIL: $(basename "$file") does not forbid raw agent IDs as primary user-facing status"
    CODEX_LABEL_ERRORS=$((CODEX_LABEL_ERRORS + 1))
  fi
done

if [ $CODEX_LABEL_ERRORS -eq 0 ]; then
  echo "PASS: Codex remora labels required"
else
  ERRORS=$((ERRORS + CODEX_LABEL_ERRORS))
fi

# 10. Check routed skills stop at interactive and terminal gates
echo ""
echo "=== Checking command boundary gates ==="
COMMAND_GATE_ERRORS=0
ROUTER="$REPO_ROOT/xavier/SKILL.md"
CODEX_ADAPTER="$REPO_ROOT/xavier/references/adapters/codex/adapter.md"
INSTALLER="$REPO_ROOT/xavier/install.sh"
SELF_UPDATE="$REPO_ROOT/xavier/skills/self-update/SKILL.md"

if ! grep -q 'Interactive gates are hard stops' "$ROUTER"; then
  echo "FAIL: router does not define interactive gates as hard stops"
  COMMAND_GATE_ERRORS=$((COMMAND_GATE_ERRORS + 1))
fi

if ! grep -q 'Terminal handoff gate' "$ROUTER"; then
  echo "FAIL: router does not define terminal handoff gate"
  COMMAND_GATE_ERRORS=$((COMMAND_GATE_ERRORS + 1))
fi

for file in "$CODEX_ADAPTER" "$INSTALLER"; do
  if ! grep -q '## Interactive Gates' "$file"; then
    echo "FAIL: $(basename "$file") does not document Codex interactive gates"
    COMMAND_GATE_ERRORS=$((COMMAND_GATE_ERRORS + 1))
  fi

  if ! grep -q 'Do not infer the answer, choose filenames, execute later steps, or invoke another Xavier command' "$file"; then
    echo "FAIL: $(basename "$file") does not forbid Codex from inferring gate answers"
    COMMAND_GATE_ERRORS=$((COMMAND_GATE_ERRORS + 1))
  fi
done

for file in "$INSTALLER" "$SELF_UPDATE"; do
  if ! grep -q 'Stop when the routed ${cmd} command reaches an AskUserQuestion/confirm/wait gate or terminal handoff' "$file"; then
    echo "FAIL: $(basename "$file") Codex alias template does not stop at routed command gates"
    COMMAND_GATE_ERRORS=$((COMMAND_GATE_ERRORS + 1))
  fi
done

for skill in grill prd research investigate tasks; do
  skill_file="$REPO_ROOT/xavier/skills/$skill/SKILL.md"
  if ! grep -q '<stop-guardrail>' "$skill_file"; then
    echo "FAIL: $skill skill missing terminal stop guardrail"
    COMMAND_GATE_ERRORS=$((COMMAND_GATE_ERRORS + 1))
  fi
done

if [ $COMMAND_GATE_ERRORS -eq 0 ]; then
  echo "PASS: command boundary gates present"
else
  ERRORS=$((ERRORS + COMMAND_GATE_ERRORS))
fi

echo ""
if [ $ERRORS -gt 0 ]; then
  echo "FAILED: $ERRORS error(s) found"
  exit 1
else
  echo "ALL CHECKS PASSED"
  exit 0
fi
