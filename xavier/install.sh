#!/bin/sh
# Xavier Installer
# Usage: curl -fsSL <url> | tar xz && bash xavier/install.sh
#   or:  git clone <repo> && cd xavier && bash install.sh
#
# Scaffolds ~/.xavier/ vault, detects runtime, wires adapter, and triggers setup.
# Requirements: git, POSIX shell. Works on macOS and Linux.

set -eu

XAVIER_HOME="${XAVIER_HOME:-$HOME/.xavier}"

# --- Resolve script directory (for symlink/copy creation) ---
# When run from a file (not piped), SCRIPT_DIR points to the xavier/ directory
SCRIPT_DIR=""
if [ -n "${0:-}" ] && [ "$0" != "sh" ] && [ "$0" != "-" ] && [ "$0" != "/dev/stdin" ] && [ -f "$0" ]; then
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
fi

# --- Detect install mode: clone (git repo) or tarball (no .git) ---
INSTALL_MODE="tarball"
if [ -n "$SCRIPT_DIR" ] && git -C "$SCRIPT_DIR" rev-parse --git-dir >/dev/null 2>&1; then
  INSTALL_MODE="clone"
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
      *)   info "Skipping vault setup. Updating symlinks..."
           install_skill
           install_command_aliases
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
- **command-aliases**: yes
- **alias-prefix**: xavier
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

# --- Detect runtimes (multi-runtime) ---
detect_runtimes() {
  info "Detecting AI agent runtimes..."

  DETECTED_RUNTIMES=""
  DETECTED_RUNTIME="unknown"

  if command -v claude >/dev/null 2>&1; then
    DETECTED_RUNTIMES="${DETECTED_RUNTIMES} claude-code"
    info "Detected: Claude Code"
  fi

  if command -v cursor >/dev/null 2>&1; then
    DETECTED_RUNTIMES="${DETECTED_RUNTIMES} cursor"
    info "Detected: Cursor"
  fi

  if command -v codex >/dev/null 2>&1; then
    DETECTED_RUNTIMES="${DETECTED_RUNTIMES} codex"
    warn "Detected: Codex (adapter not yet available — will use stub)"
  fi

  # Trim leading space
  DETECTED_RUNTIMES="$(echo "$DETECTED_RUNTIMES" | sed 's/^ //')"

  if [ -z "$DETECTED_RUNTIMES" ]; then
    warn "No known AI agent runtime detected."
    warn "Xavier will work but agent spawning will be limited."
  else
    # Use the first runtime that has an adapter implementation as primary
    for rt in $DETECTED_RUNTIMES; do
      if [ "$rt" = "claude-code" ] || [ "$rt" = "cursor" ]; then
        DETECTED_RUNTIME="$rt"
        break
      fi
    done
    info "Primary runtime: $DETECTED_RUNTIME"
  fi
}

# --- Wire adapters for all detected runtimes ---
wire_adapters() {
  if [ -z "$DETECTED_RUNTIMES" ]; then
    return 0
  fi

  for runtime in $DETECTED_RUNTIMES; do
    wire_single_adapter "$runtime"
  done

  # Update config with primary runtime and list available adapters
  if command -v sed >/dev/null 2>&1; then
    sed -i.bak "s/- \*\*adapter\*\*: .*/- **adapter**: $DETECTED_RUNTIME/" "$XAVIER_HOME/config.md" 2>/dev/null && rm -f "$XAVIER_HOME/config.md.bak"
  fi

  # Append available-adapters line if multiple runtimes detected
  RUNTIME_COUNT="$(echo "$DETECTED_RUNTIMES" | wc -w | tr -d ' ')"
  if [ "$RUNTIME_COUNT" -gt 1 ]; then
    ADAPTERS_LIST="$(echo "$DETECTED_RUNTIMES" | tr ' ' ', ')"
    if ! grep -q "available-adapters" "$XAVIER_HOME/config.md" 2>/dev/null; then
      sed -i.bak "s/- \*\*adapter\*\*: .*/- **adapter**: $DETECTED_RUNTIME\n- **available-adapters**: [$ADAPTERS_LIST]/" "$XAVIER_HOME/config.md" 2>/dev/null && rm -f "$XAVIER_HOME/config.md.bak"
    fi
  fi
}

wire_single_adapter() {
  runtime="$1"

  if [ "$runtime" != "claude-code" ] && [ "$runtime" != "cursor" ]; then
    warn "No adapter implementation for $runtime — skipping"
    return 0
  fi

  info "Wiring $runtime adapter..."
  mkdir -p "$XAVIER_HOME/adapters/$runtime"

  if [ "$runtime" = "claude-code" ]; then
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

  if [ "$runtime" = "cursor" ]; then
    cat > "$XAVIER_HOME/adapters/cursor/adapter.md" << 'ADAPTEREOF'
---
name: cursor
type: adapter
runtime: cursor
---

# Cursor Runtime Adapter

## spawn(task, options) -> handle
Use the Task tool with subagent_type: "generalPurpose", run_in_background: true. The handle is the task ID.

## poll(handle) -> status
Use Await(task_id: handle) or read the output_file. Check for exit_code footer to determine completion. Use incremental backoff (2s, 4s, 8s...).

## collect(tasks[]) -> results[]
Spawn all tasks in a single message (parallel Task tool calls) with run_in_background: true. Poll each handle to completion.
ADAPTEREOF
  fi

  info "Adapter wired: $runtime"
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

# --- Register skill symlinks/copies ---
install_skill() {
  if [ -z "$SCRIPT_DIR" ]; then
    warn "Script was piped (curl | sh) — cannot create skill symlinks."
    warn "Run /xavier setup from your AI agent to register symlinks, or"
    warn "re-run the installer directly: sh /path/to/xavier/install.sh"
    return 0
  fi

  info "Registering Xavier skill ($INSTALL_MODE mode)..."

  # Symlink 1: ~/.agents/skills/xavier/ -> $SCRIPT_DIR (the xavier/ directory)
  create_symlink "$HOME/.agents/skills/xavier" "$SCRIPT_DIR" "$HOME/.agents/skills"

  # Determine SKILL.md source based on install mode
  if [ "$INSTALL_MODE" = "clone" ]; then
    SKILL_SOURCE="$SCRIPT_DIR/SKILL.md"
  else
    # Tarball mode: copy SKILL.md into XAVIER_HOME so it persists
    if [ -f "$SCRIPT_DIR/SKILL.md" ]; then
      cp "$SCRIPT_DIR/SKILL.md" "$XAVIER_HOME/SKILL.md"
      info "Copied SKILL.md to $XAVIER_HOME/SKILL.md"
    fi
    SKILL_SOURCE="$XAVIER_HOME/SKILL.md"
  fi

  # Symlink 2: ~/.claude/commands/xavier.md -> SKILL.md (Claude Code)
  create_symlink "$HOME/.claude/commands/xavier.md" "$SKILL_SOURCE" "$HOME/.claude/commands"

  # Symlink 3: ~/.claude/commands/x.md -> SKILL.md (Claude Code short alias)
  create_symlink "$HOME/.claude/commands/x.md" "$SKILL_SOURCE" "$HOME/.claude/commands"

  # Cursor: per-command aliases handle discoverability (installed by install_command_aliases)
}

# --- Helper: create a symlink with broken-link cleanup ---
create_symlink() {
  link_path="$1"
  target="$2"
  parent_dir="$3"

  if [ -L "$link_path" ] && [ ! -e "$link_path" ]; then
    warn "Removing broken symlink: $link_path"
    rm "$link_path"
  fi
  if [ -e "$link_path" ]; then
    warn "Symlink already exists: $link_path — skipping"
  else
    mkdir -p "$parent_dir"
    ln -s "$target" "$link_path"
    info "Created: $link_path -> $target"
  fi
}

# --- Generate per-command aliases for Claude Code and Cursor ---
install_command_aliases() {
  if [ -z "$SCRIPT_DIR" ]; then
    return 0
  fi

  # Check config for command-aliases preference (default: yes)
  ALIASES_ENABLED="yes"
  if [ -f "$XAVIER_HOME/config.md" ]; then
    config_val="$(grep -o '\*\*command-aliases\*\*: *[a-zA-Z]*' "$XAVIER_HOME/config.md" 2>/dev/null | head -n 1 | awk -F': *' '{print $2}')"
    config_val="$(echo "$config_val" | tr '[:upper:]' '[:lower:]')"
    if [ "$config_val" = "no" ] || [ "$config_val" = "false" ]; then
      ALIASES_ENABLED="no"
    fi
  fi

  if [ "$ALIASES_ENABLED" = "no" ]; then
    info "Command aliases disabled in config — skipping."
    return 0
  fi

  # Read alias prefix from config (default: xavier)
  ALIAS_PREFIX="xavier"
  if [ -f "$XAVIER_HOME/config.md" ]; then
    prefix_val="$(grep -o '\*\*alias-prefix\*\*: *[^ ]*' "$XAVIER_HOME/config.md" 2>/dev/null | head -n 1 | awk -F': *' '{print $2}')"
    if [ -n "$prefix_val" ]; then
      if printf '%s' "$prefix_val" | grep -qE '^[a-zA-Z0-9_-]+$'; then
        ALIAS_PREFIX="$prefix_val"
      else
        warn "Invalid alias-prefix '$prefix_val' — must be alphanumeric, hyphens, or underscores. Falling back to 'xavier'."
      fi
    fi
  fi

  info "Generating per-command aliases (prefix: $ALIAS_PREFIX)..."

  # Command descriptions for alias files
  # Format: command|description
  COMMANDS="
setup|Create and configure the Xavier vault
review|Run Shark-pattern code review with concurrent reviewer personas
babysit|Monitor a PR — poll CI status, auto-fix lint failures, surface review comments
grill|Interview about a plan or design until reaching shared understanding
prd|Create a PRD through user interview, codebase exploration, and module design
tasks|Decompose a PRD into phased implementation tasks
learn|Explore a codebase and produce knowledge notes in the vault
loop|Execute a task file as an autonomous loop using the Shark pattern
add-dep|Create a dependency-skill for a package with best practices and API patterns
remove-dep|Delete a dependency-skill
deps-update|Scan lockfile and regenerate stale dependency-skills
export|Export a vault note to your personal Obsidian vault
self-update|Update Xavier skills and references to the latest release
uninstall|Remove the Xavier vault and all symlinks
"

  echo "$COMMANDS" | while IFS='|' read -r cmd desc; do
    [ -z "$cmd" ] && continue

    # Claude Code: ~/.claude/commands/<prefix>-<cmd>.md
    claude_alias="$HOME/.claude/commands/${ALIAS_PREFIX}-${cmd}.md"
    if [ ! -e "$claude_alias" ]; then
      mkdir -p "$HOME/.claude/commands"
      cat > "$claude_alias" << ALIASEOF
---
name: ${ALIAS_PREFIX}-${cmd}
description: ${desc}
---

Run /xavier ${cmd} — load and follow the xavier skill router.
ALIASEOF
    fi

    # Cursor: ~/.cursor/skills/<prefix>-<cmd>/SKILL.md
    cursor_alias="$HOME/.cursor/skills/${ALIAS_PREFIX}-${cmd}/SKILL.md"
    if [ ! -e "$cursor_alias" ]; then
      mkdir -p "$HOME/.cursor/skills/${ALIAS_PREFIX}-${cmd}"
      cat > "$cursor_alias" << ALIASEOF
---
name: ${ALIAS_PREFIX}-${cmd}
description: "${desc}. Use when user says /xavier ${cmd}."
---

Execute /xavier ${cmd}.

1. Read the Xavier router from \${XAVIER_HOME:-~/.xavier}/SKILL.md (or ~/.xavier/SKILL.md if unset)
2. Follow the Router Lifecycle with subcommand: ${cmd}
ALIASEOF
    fi
  done

  info "Command aliases installed for Claude Code and Cursor."
}

# --- Symlink or copy skills & references into ~/.xavier/ ---
link_xavier_skills_and_refs() {
  if [ -z "$SCRIPT_DIR" ]; then
    warn "Script was piped (curl | sh) — cannot create $XAVIER_HOME symlinks."
    warn "Re-run the installer directly: sh /path/to/xavier/install.sh"
    return 0
  fi

  if [ "$INSTALL_MODE" = "clone" ]; then
    info "Linking skills and references into $XAVIER_HOME (clone mode)..."

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

    # --- Symlink each dep directory (distributed deps only) ---
    if [ -d "$SCRIPT_DIR/deps" ]; then
      mkdir -p "$XAVIER_HOME/deps"
      for dep_dir in "$SCRIPT_DIR/deps/"*/; do
        [ -d "$dep_dir" ] || continue
        dep_name="$(basename "$dep_dir")"
        dep_target="$XAVIER_HOME/deps/$dep_name"
        # If target is a real directory (not a symlink), move it aside
        # so ln -sfn doesn't create the link inside it
        if [ -d "$dep_target" ] && [ ! -L "$dep_target" ]; then
          mv "$dep_target" "${dep_target}.prev"
          warn "Moved existing dep directory to ${dep_name}.prev — remove with: rm -r \"${dep_target}.prev\""
        fi
        ln -sfn "$dep_dir" "$dep_target"
        info "  dep: $dep_name -> $dep_dir"
      done
    fi

    info "Skills, references, and deps linked."

  else
    info "Copying skills and references into $XAVIER_HOME (tarball mode)..."

    # --- Copy each skill directory (replace existing) ---
    mkdir -p "$XAVIER_HOME/skills"
    for skill_dir in "$SCRIPT_DIR/skills/"*/; do
      [ -d "$skill_dir" ] || continue
      skill_name="$(basename "$skill_dir")"
      # Remove existing (could be old symlink or directory)
      rm -rf "$XAVIER_HOME/skills/$skill_name"
      cp -R "$skill_dir" "$XAVIER_HOME/skills/$skill_name"
      info "  skill: $skill_name (copied)"
    done

    # --- Copy references directory (replace existing) ---
    if [ -d "$SCRIPT_DIR/references" ]; then
      # Remove existing (could be old symlink or directory)
      rm -rf "$XAVIER_HOME/references"
      cp -R "$SCRIPT_DIR/references" "$XAVIER_HOME/references"
      info "  references (copied)"
    fi

    # --- Copy each dep directory (merge — only replace distributed deps) ---
    if [ -d "$SCRIPT_DIR/deps" ]; then
      mkdir -p "$XAVIER_HOME/deps"
      for dep_dir in "$SCRIPT_DIR/deps/"*/; do
        [ -d "$dep_dir" ] || continue
        dep_name="$(basename "$dep_dir")"
        rm -rf "$XAVIER_HOME/deps/$dep_name"
        cp -R "$dep_dir" "$XAVIER_HOME/deps/$dep_name"
        info "  dep: $dep_name (copied)"
      done
    fi

    info "Skills, references, and deps copied."
  fi
}

# --- Summary ---
print_summary() {
  echo ""
  bold "Xavier installed successfully!"
  echo ""
  info "Vault location: $XAVIER_HOME"
  if [ -n "$DETECTED_RUNTIMES" ]; then
    info "Detected runtimes: $DETECTED_RUNTIMES"
    info "Primary adapter: $DETECTED_RUNTIME"
  else
    info "Runtime: none detected"
  fi
  info "Install mode: $INSTALL_MODE"
  echo ""

  if [ "$INSTALL_MODE" = "clone" ]; then
    SKILL_REF_NOTE="symlinked from repo"
  else
    SKILL_REF_NOTE="copied from tarball"
  fi

  echo "  Directory structure:"
  echo "  $XAVIER_HOME/"
  echo "  ├── config.md"
  echo "  ├── MEMORY.md"
  echo "  ├── personas/"
  echo "  ├── adapters/ (${DETECTED_RUNTIMES:-none})"
  echo "  ├── skills/ ($SKILL_REF_NOTE)"
  echo "  ├── references/ ($SKILL_REF_NOTE)"
  echo "  ├── knowledge/{repos,teams,reviews}/"
  echo "  ├── prd/"
  echo "  ├── tasks/"
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
  DETECTED_RUNTIMES=""

  if [ "$EXISTING" = "false" ]; then
    scaffold_vault
  fi

  detect_runtimes
  wire_adapters

  if [ "$EXISTING" = "false" ]; then
    init_git
  fi

  install_skill
  install_command_aliases
  link_xavier_skills_and_refs
  print_summary
}

main "$@"
