#!/bin/sh
# Xavier Installer
# Usage: curl -fsSL <url> | sh
#
# Scaffolds ~/.xavier/ vault, detects runtime, wires adapter, and triggers setup.
# Requirements: git, POSIX shell. Works on macOS and Linux.

set -eu

XAVIER_HOME="${XAVIER_HOME:-$HOME/.xavier}"

# --- Resolve script directory (for symlink creation) ---
# When run from a file (not piped), SCRIPT_DIR points to the xavier/ directory
SCRIPT_DIR=""
if [ -n "${0:-}" ] && [ "$0" != "sh" ] && [ "$0" != "-" ] && [ "$0" != "/dev/stdin" ] && [ -f "$0" ]; then
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
fi

# --- Colors (if terminal supports them) ---
if [ -t 1 ]; then
  BOLD='\033[1m'
  GREEN='\033[0;32m'
  YELLOW='\033[0;33m'
  RED='\033[0;31m'
  RESET='\033[0m'
else
  BOLD='' GREEN='' YELLOW='' RED='' RESET=''
fi

info()  { printf "${GREEN}[xavier]${RESET} %s\n" "$1"; }
warn()  { printf "${YELLOW}[xavier]${RESET} %s\n" "$1"; }
error() { printf "${RED}[xavier]${RESET} %s\n" "$1" >&2; }
bold()  { printf "${BOLD}%s${RESET}\n" "$1"; }

# --- Pre-flight checks ---
check_deps() {
  for cmd in git; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
      error "Required command not found: $cmd"
      error "Please install $cmd and re-run the installer."
      exit 1
    fi
  done
}

# --- Check for existing installation ---
check_existing() {
  if [ -d "$XAVIER_HOME" ] && [ -f "$XAVIER_HOME/config.md" ]; then
    warn "Xavier vault already exists at $XAVIER_HOME"
    printf "  [u] Update — re-run setup to update preferences\n"
    printf "  [s] Skip — exit without changes\n"
    printf "  Choice [u/s]: "
    read -r choice
    case "$choice" in
      u|U) info "Will re-run setup after scaffold check..." ;;
      *)   info "Skipping vault setup. Updating skill symlinks..."
           link_xavier_skills_and_refs
           exit 0 ;;
    esac
    return 0
  fi
  return 1
}

# --- Scaffold vault directory structure ---
scaffold_vault() {
  info "Creating vault at $XAVIER_HOME..."

  mkdir -p "$XAVIER_HOME/personas"
  mkdir -p "$XAVIER_HOME/adapters"
  mkdir -p "$XAVIER_HOME/skills"
  mkdir -p "$XAVIER_HOME/knowledge/repos"
  mkdir -p "$XAVIER_HOME/knowledge/teams"
  mkdir -p "$XAVIER_HOME/knowledge/reviews"
  mkdir -p "$XAVIER_HOME/prd"
  mkdir -p "$XAVIER_HOME/tasks"
  mkdir -p "$XAVIER_HOME/review-state"
  mkdir -p "$XAVIER_HOME/loop-state"
  mkdir -p "$XAVIER_HOME/shark-state"

  # Write minimal config.md (will be personalized by /xavier setup)
  if [ ! -f "$XAVIER_HOME/config.md" ]; then
    cat > "$XAVIER_HOME/config.md" << 'CONFIGEOF'
---
version: 1
---

# Xavier Configuration

## User

- **name**: (not yet configured)
- **teams**: []

## Preferences

- **git-strategy**: batch-commit
- **workflow**: (not yet configured)
- **review-priorities**: balanced

## Runtime

- **adapter**: (not yet detected)
CONFIGEOF
  fi

  # Write MEMORY.md
  if [ ! -f "$XAVIER_HOME/MEMORY.md" ]; then
    cat > "$XAVIER_HOME/MEMORY.md" << 'MEMEOF'
# Xavier Memory Index

_No memories yet. Xavier will populate this as it learns about your codebase and preferences._
MEMEOF
  fi

  info "Vault directory structure created."
}

# --- Detect runtime ---
detect_runtime() {
  info "Detecting AI agent runtime..."

  # Check for Claude Code
  if command -v claude >/dev/null 2>&1; then
    DETECTED_RUNTIME="claude-code"
    info "Detected: Claude Code"
    return 0
  fi

  # Check for Codex (stub)
  if command -v codex >/dev/null 2>&1; then
    DETECTED_RUNTIME="codex"
    warn "Detected: Codex (adapter not yet available — will use stub)"
    return 0
  fi

  DETECTED_RUNTIME="unknown"
  warn "No known AI agent runtime detected."
  warn "Xavier will work but agent spawning will be limited."
  return 0
}

# --- Wire adapter ---
wire_adapter() {
  if [ "$DETECTED_RUNTIME" = "unknown" ]; then
    return 0
  fi

  info "Wiring $DETECTED_RUNTIME adapter..."
  mkdir -p "$XAVIER_HOME/adapters/$DETECTED_RUNTIME"

  # For claude-code, write the adapter instruction file
  if [ "$DETECTED_RUNTIME" = "claude-code" ]; then
    cat > "$XAVIER_HOME/adapters/claude-code/adapter.md" << 'ADAPTEREOF'
---
name: claude-code
type: adapter
runtime: claude-code
---

# Claude Code Runtime Adapter

## spawn(task, options) -> handle
Use the Agent tool with run_in_background: true. The handle is the agent ID.

## poll(handle) -> status
Claude Code auto-notifies on completion. No explicit polling needed.

## collect(handles[]) -> results[]
Spawn all agents in a single message (parallel tool calls), wait for notifications.
ADAPTEREOF
  fi

  # Update config with detected runtime
  if command -v sed >/dev/null 2>&1; then
    sed -i.bak "s/- \*\*adapter\*\*: .*/- **adapter**: $DETECTED_RUNTIME/" "$XAVIER_HOME/config.md" 2>/dev/null && rm -f "$XAVIER_HOME/config.md.bak"
  fi

  info "Adapter wired: $DETECTED_RUNTIME"
}

# --- Initialize git ---
init_git() {
  if [ -d "$XAVIER_HOME/.git" ]; then
    info "Git repository already initialized."
    return 0
  fi

  info "Initializing git repository..."
  (
    cd "$XAVIER_HOME"
    git init -q
    git add -A
    git commit -q -m "xavier: initial vault scaffold"
  )
  info "Git repository initialized with initial commit."
}

# --- Register skill symlinks ---
install_skill() {
  if [ -z "$SCRIPT_DIR" ]; then
    warn "Script was piped (curl | sh) — cannot create skill symlinks."
    warn "Run /xavier setup from Claude Code to register symlinks, or"
    warn "re-run the installer directly: sh /path/to/xavier/install.sh"
    return 0
  fi

  info "Registering Xavier skill symlinks..."

  # Symlink 1: ~/.agents/skills/xavier/ -> $SCRIPT_DIR (the xavier/ directory)
  AGENTS_LINK="$HOME/.agents/skills/xavier"
  if [ -e "$AGENTS_LINK" ] || [ -L "$AGENTS_LINK" ]; then
    warn "Symlink already exists: $AGENTS_LINK — skipping"
  else
    mkdir -p "$HOME/.agents/skills"
    ln -s "$SCRIPT_DIR" "$AGENTS_LINK"
    info "Created: $AGENTS_LINK -> $SCRIPT_DIR"
  fi

  # Symlink 2: ~/.claude/commands/xavier.md -> $SCRIPT_DIR/SKILL.md
  COMMANDS_LINK="$HOME/.claude/commands/xavier.md"
  if [ -e "$COMMANDS_LINK" ] || [ -L "$COMMANDS_LINK" ]; then
    warn "Symlink already exists: $COMMANDS_LINK — skipping"
  else
    mkdir -p "$HOME/.claude/commands"
    ln -s "$SCRIPT_DIR/SKILL.md" "$COMMANDS_LINK"
    info "Created: $COMMANDS_LINK -> $SCRIPT_DIR/SKILL.md"
  fi
}

# --- Symlink skills & references into ~/.xavier/ ---
link_xavier_skills_and_refs() {
  if [ -z "$SCRIPT_DIR" ]; then
    warn "Script was piped (curl | sh) — cannot create $XAVIER_HOME symlinks."
    warn "Re-run the installer directly: sh /path/to/xavier/install.sh"
    return 0
  fi

  info "Linking skills and references into $XAVIER_HOME..."

  # --- Clean up broken symlinks in ~/.xavier/skills/ ---
  if [ -d "$XAVIER_HOME/skills" ]; then
    for link in "$XAVIER_HOME/skills/"*; do
      [ -L "$link" ] && [ ! -e "$link" ] && {
        warn "Removing broken symlink: $link"
        rm "$link"
      }
    done
  fi

  # --- Clean up broken symlink at ~/.xavier/references ---
  if [ -L "$XAVIER_HOME/references" ] && [ ! -e "$XAVIER_HOME/references" ]; then
    warn "Removing broken symlink: $XAVIER_HOME/references"
    rm "$XAVIER_HOME/references"
  fi

  # --- Symlink each skill directory ---
  mkdir -p "$XAVIER_HOME/skills"
  for skill_dir in "$SCRIPT_DIR/skills/"*/; do
    [ -d "$skill_dir" ] || continue
    skill_name="$(basename "$skill_dir")"
    ln -sfn "$skill_dir" "$XAVIER_HOME/skills/$skill_name"
    info "  skill: $skill_name -> $skill_dir"
  done

  # --- Symlink references directory ---
  ln -sfn "$SCRIPT_DIR/references" "$XAVIER_HOME/references"
  info "  references -> $SCRIPT_DIR/references"

  info "Skills and references linked."
}

# --- Summary ---
print_summary() {
  echo ""
  bold "Xavier installed successfully!"
  echo ""
  info "Vault location: $XAVIER_HOME"
  info "Runtime: $DETECTED_RUNTIME"
  echo ""
  echo "  Directory structure:"
  echo "  $XAVIER_HOME/"
  echo "  ├── config.md"
  echo "  ├── MEMORY.md"
  echo "  ├── personas/"
  echo "  ├── adapters/$DETECTED_RUNTIME/"
  echo "  ├── skills/ (symlinked from repo)"
  echo "  ├── references/ (symlinked from repo)"
  echo "  ├── knowledge/{repos,teams,reviews}/"
  echo "  ├── prd/"
  echo "  ├── tasks/"
  echo "  ├── review-state/"
  echo "  ├── loop-state/"
  echo "  └── shark-state/"
  echo ""
  bold "Next steps:"
  echo "  1. Run /xavier setup in your AI agent to personalize"
  echo "  2. Run /xavier review on any repo to start reviewing"
  echo ""
  info "To push your vault to GitHub:"
  echo "  cd $XAVIER_HOME && gh repo create xavier-ai --private --source=. --push"
  echo ""
}

# --- Main ---
main() {
  bold "Xavier Installer"
  echo ""

  check_deps

  EXISTING=false
  if check_existing; then
    EXISTING=true
  fi

  DETECTED_RUNTIME="unknown"

  if [ "$EXISTING" = "false" ]; then
    scaffold_vault
  fi

  detect_runtime
  wire_adapter

  if [ "$EXISTING" = "false" ]; then
    init_git
  fi

  install_skill
  link_xavier_skills_and_refs
  print_summary
}

main "$@"
