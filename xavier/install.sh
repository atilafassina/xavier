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
    while :; do
      printf "  [u] Update — re-run the rest of install.sh against the existing vault.\n"
      printf "                  config.md is preserved as-is; rerun /xavier setup separately\n"
      printf "                  if you want to update preferences interactively\n"
      printf "  [s] Refresh-only — keep existing config.md; create any missing vault\n"
      printf "                  directories, re-detect runtimes, refresh skill symlinks,\n"
      printf "                  and regenerate command aliases. Skips later install steps.\n"
      printf "  [a] Abort — exit without changes\n"
      printf "  Choice [u/s/a]: "
      read -r choice
      case "$choice" in
        u|U) info "Will re-run setup after scaffold check..."
             return 0 ;;
        s|S) info "Refreshing vault layout, symlinks, adapters, and aliases (config.md preserved)..."
             ensure_vault_dirs
             # Re-detect runtimes and re-wire adapters so adapter contract changes
             # and newly-installed runtimes (e.g. user installed Cursor since last
             # run) land on the same `s`-path as a fresh install would produce.
             detect_runtimes
             PRESERVE_CONFIG=true
             wire_adapters
             PRESERVE_CONFIG=false
             install_skill
             install_command_aliases
             link_xavier_skills_and_refs
             select_native_tool
             exit 0 ;;
        a|A) info "Aborted. No changes made."
             exit 0 ;;
        *)   warn "Invalid choice '$choice'. Pick u, s, or a." ;;
      esac
    done
  fi
  return 1
}

# --- Scaffold vault directory structure ---
# Idempotent — creates any missing vault directories. Runs on fresh installs AND
# upgrades so new layout requirements (e.g. prd/done, tasks/done) materialize for
# vaults that predate them. This list is the single source of truth for the vault
# scaffold; xavier/skills/self-update/SKILL.md Step 10 mirrors it line-for-line.
ensure_vault_dirs() {
  mkdir -p "$XAVIER_HOME/personas"
  mkdir -p "$XAVIER_HOME/adapters"
  mkdir -p "$XAVIER_HOME/skills"
  mkdir -p "$XAVIER_HOME/deps"
  mkdir -p "$XAVIER_HOME/knowledge/repos"
  mkdir -p "$XAVIER_HOME/knowledge/teams"
  mkdir -p "$XAVIER_HOME/knowledge/reviews"
  mkdir -p "$XAVIER_HOME/knowledge/cohorts"
  mkdir -p "$XAVIER_HOME/prd"
  mkdir -p "$XAVIER_HOME/prd/done"
  mkdir -p "$XAVIER_HOME/tasks"
  mkdir -p "$XAVIER_HOME/tasks/done"
  mkdir -p "$XAVIER_HOME/research"
  mkdir -p "$XAVIER_HOME/investigations"
  mkdir -p "$XAVIER_HOME/loop-state"
  mkdir -p "$XAVIER_HOME/teach-state"
  mkdir -p "$XAVIER_HOME/shark-state"
  mkdir -p "$XAVIER_HOME/babysit-pr"
}

scaffold_vault() {
  info "Creating vault at $XAVIER_HOME..."

  ensure_vault_dirs

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
    info "Detected: Codex"
  fi

  # Trim leading space
  DETECTED_RUNTIMES="$(echo "$DETECTED_RUNTIMES" | sed 's/^ //')"

  if [ -z "$DETECTED_RUNTIMES" ]; then
    warn "No known AI agent runtime detected."
    warn "Xavier will work but agent spawning will be limited."
  else
    # Use the first runtime that has an adapter implementation as primary
    for rt in $DETECTED_RUNTIMES; do
      if [ "$rt" = "claude-code" ] || [ "$rt" = "cursor" ] || [ "$rt" = "codex" ]; then
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

  RUNTIME_COUNT="$(echo "$DETECTED_RUNTIMES" | wc -w | tr -d ' ')"
  ADAPTERS_LIST="$(echo "$DETECTED_RUNTIMES" | sed 's/ /, /g')"

  if [ "${PRESERVE_CONFIG:-false}" = "true" ]; then
    # Preserve the user's primary `adapter:` selection, but always refresh
    # `available-adapters:` to reflect what's currently detected. Without
    # this, a Claude-only vault that gains Cursor/Codex via [s] refresh
    # would wire the new adapter.md files but never advertise them in
    # config.md — forcing the user back through /xavier setup just to
    # switch primary.
    info "Preserving primary adapter; refreshing available-adapters from detection."
    refresh_available_adapters
    return 0
  fi

  if command -v sed >/dev/null 2>&1; then
    sed -i.bak "s/- \*\*adapter\*\*: .*/- **adapter**: $DETECTED_RUNTIME/" "$XAVIER_HOME/config.md" 2>/dev/null && rm -f "$XAVIER_HOME/config.md.bak"
  fi

  refresh_available_adapters
}

# Insert or update `- **available-adapters**:` in config.md to match the
# currently detected runtime set. Single-runtime installs keep the field
# absent unless it already exists (in which case it's refreshed in place).
# Uses awk for line insertion — BSD sed and GNU sed disagree on `\n` in
# the replacement string, so the prior `s/.../...\n.../` form silently
# inserted a literal `\n` on macOS.
refresh_available_adapters() {
  if grep -q "available-adapters" "$XAVIER_HOME/config.md" 2>/dev/null; then
    sed -i.bak "s/- \*\*available-adapters\*\*: .*/- **available-adapters**: [$ADAPTERS_LIST]/" "$XAVIER_HOME/config.md" 2>/dev/null && rm -f "$XAVIER_HOME/config.md.bak"
  elif [ "$RUNTIME_COUNT" -gt 1 ]; then
    awk -v list="$ADAPTERS_LIST" '
      /^- \*\*adapter\*\*:/ { print; print "- **available-adapters**: [" list "]"; next }
      { print }
    ' "$XAVIER_HOME/config.md" > "$XAVIER_HOME/config.md.tmp" && mv "$XAVIER_HOME/config.md.tmp" "$XAVIER_HOME/config.md"
  fi
}

wire_single_adapter() {
  runtime="$1"

  if [ "$runtime" != "claude-code" ] && [ "$runtime" != "cursor" ] && [ "$runtime" != "codex" ]; then
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

  if [ "$runtime" = "codex" ]; then
    cat > "$XAVIER_HOME/adapters/codex/adapter.md" << 'ADAPTEREOF'
---
name: codex
type: adapter
runtime: codex
---

# Codex Runtime Adapter

## spawn(task, options) -> handle
Use the spawn_agent tool. Prefer agent_type: "explorer" for research and codebase discovery, "worker" for implementation and test-fixing tasks, and "default" for general tasks. Do not set model unless the user explicitly asks or the task clearly requires it.

Prefix the spawned message with `Xavier remora: {options.name or derived task label}`. The raw spawn_agent ID is an internal handle only. Track each remora as `{ label, nickname, handle }`, preferring `options.name` for the label and recording the Codex nickname returned by spawn_agent.

If spawn_agent is unavailable in the current Codex session, warn once: "Codex subagents unavailable; running inline, so Shark parallelism is disabled." Then run the task inline.

## collect(tasks[]) -> results[]
Spawn independent tasks concurrently in a single parallel tool-call batch. Use explorer agents for read-only research tasks and worker agents for implementation tasks. Ensure every task has a user-visible label from `task.name` or a derived task purpose, then maintain an agent map of `{ label, nickname, handle }`.

Collect results with wait_agent. Before any blocking wait_agent call, print the remora labels being waited on, for example `Waiting for 3 remoras: Foundations; Tools comparison; Local context.` Never present raw agent hashes as the primary user-facing status list. If Codex displays handles anyway, immediately follow with the label map so the user can interpret them.

If subagents are unavailable, run tasks inline one at a time and preserve the same warning behavior as spawn().

## poll(handle) -> status
Use wait_agent(handle). Codex also sends completion notifications, but wait_agent is the explicit backpressure point when the next step depends on a remora result. Resolve handles through the agent map and announce remora labels before polling; do not say only "waiting for 019e...".

## Interactive Gates
Codex executes Xavier router and skill instructions inline, so interactive gates must be treated as hard command boundaries. Whenever a routed skill says `AskUserQuestion`, ask, prompt, confirm, quiz, wait for the user, or get feedback, Codex must ask the user and stop. Do not infer the answer, choose filenames, execute later steps, or invoke another Xavier command until the user replies.

When a skill reaches a terminal handoff, show the suggested next commands as options only. Do not automatically move from `grill` to `prd`, from `prd` to `tasks`, from `tasks` to `loop`, or from any Xavier skill into code edits unless the user's newest message explicitly asks for that command.

## Tool Dispatch

| Operation | Tool |
|-----------|------|
| run-command | exec_command |
| read-file | exec_command with sed/nl or shell file reads |
| write-file | apply_patch |
| spawn-agent | spawn_agent |
| poll-agent | wait_agent |
| search-text | exec_command with rg |
| search-files | exec_command with rg --files |
ADAPTEREOF
  fi

  info "Adapter wired: $runtime"
}

# --- Detect host target triple ---
# Maps the full `uname -s`/`uname -m` matrix to the Rust target triples used
# for the bundled binary layout (xavier/bin/<triple>/xavier-tool). Mirrors what
# `rustc -vV` would report on each platform, without requiring a Rust toolchain
# on the user's machine.
#
# This map MUST stay in lockstep with the set of triples cross-compiled by
# .github/workflows/release.yml, the runtime resolver in
# deps/multi-model-dispatch/merge.sh, and the offline guard
# validate-install-triples.sh. The shipped set is:
#   {x86_64, aarch64} x {apple-darwin (Darwin), unknown-linux-gnu (Linux)}
#
# Echoes the triple, or NOTHING for any unsupported os/arch combination
# (e.g. 32-bit x86, armv7, FreeBSD/Windows-via-uname) so the caller falls back
# to the pure-shell merge instead of selecting a binary we never built.
detect_host_triple() {
  os="$(uname -s 2>/dev/null || echo unknown)"
  arch="$(uname -m 2>/dev/null || echo unknown)"

  # Normalize architecture names to Rust's vocabulary. Anything we do NOT ship
  # a binary for is left empty so the os case below yields no triple.
  case "$arch" in
    x86_64|amd64)  rust_arch="x86_64" ;;
    arm64|aarch64) rust_arch="aarch64" ;;
    *)             rust_arch="" ;;
  esac

  # Unsupported architecture → no triple (graceful shell fallback).
  if [ -z "$rust_arch" ]; then
    echo ""
    return 0
  fi

  case "$os" in
    Darwin)  echo "${rust_arch}-apple-darwin" ;;
    Linux)   echo "${rust_arch}-unknown-linux-gnu" ;;
    *)       echo "" ;;
  esac
}

# --- Select the bundled native tool for this host ---
# Gated, like every per-runtime artifact: if a binary for the host triple is
# bundled, install it into the vault at a stable, runtime-resolvable path
# ($XAVIER_HOME/bin/<triple>/xavier-tool) and mark it executable. If none
# matches (unsupported platform, or a clone checkout whose bin/ holds no
# prebuilt binaries), NO-OP — never write a stub. Skills fall back to the
# pure-shell parse.sh merge when the binary is absent.
#
# Source binaries live under $SCRIPT_DIR/bin (the extracted tarball / repo);
# clone mode symlinks, tarball mode copies — matching link_xavier_skills_and_refs.
select_native_tool() {
  if [ -z "$SCRIPT_DIR" ]; then
    return 0
  fi

  src_root="$SCRIPT_DIR/bin"
  if [ ! -d "$src_root" ]; then
    return 0
  fi

  triple="$(detect_host_triple)"
  if [ -z "$triple" ]; then
    warn "Unsupported platform for native tool ($(uname -s 2>/dev/null)/$(uname -m 2>/dev/null)) — using shell fallback."
    return 0
  fi

  src_tool="$src_root/$triple/xavier-tool"
  if [ ! -f "$src_tool" ]; then
    info "No bundled native tool for $triple — using shell fallback."
    return 0
  fi

  dest_dir="$XAVIER_HOME/bin/$triple"
  dest_tool="$dest_dir/xavier-tool"
  mkdir -p "$dest_dir"

  if [ "$INSTALL_MODE" = "clone" ]; then
    ln -sfn "$src_tool" "$dest_tool"
  else
    cp "$src_tool" "$dest_tool"
  fi
  chmod +x "$dest_tool" 2>/dev/null || true
  info "Native tool installed: $dest_tool"
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

  # Codex base skill: ~/.agents/skills/xavier/ -> $SCRIPT_DIR
  # Only created when Codex is detected — otherwise a Claude-only or
  # Cursor-only user accumulates stale entries in Codex's skill root.
  case " $DETECTED_RUNTIMES " in
    *" codex "*)
      create_symlink "$HOME/.agents/skills/xavier" "$SCRIPT_DIR" "$HOME/.agents/skills"
      ;;
  esac

  # Determine SKILL.md source based on install mode. The vault-router
  # symlink at $XAVIER_HOME/SKILL.md is kept unconditional: Cursor and
  # Codex aliases read from it, and a missing symlink in clone mode
  # leaves refresh installs reading stale router logic.
  if [ "$INSTALL_MODE" = "clone" ]; then
    SKILL_SOURCE="$SCRIPT_DIR/SKILL.md"
    ln -sfn "$SKILL_SOURCE" "$XAVIER_HOME/SKILL.md"
    info "Linked SKILL.md to $XAVIER_HOME/SKILL.md"
  else
    if [ -f "$SCRIPT_DIR/SKILL.md" ]; then
      cp "$SCRIPT_DIR/SKILL.md" "$XAVIER_HOME/SKILL.md"
      info "Copied SKILL.md to $XAVIER_HOME/SKILL.md"
    fi
    SKILL_SOURCE="$XAVIER_HOME/SKILL.md"
  fi

  # Claude Code base + short alias — gated so users without `claude` on
  # PATH don't get phantom command entries.
  case " $DETECTED_RUNTIMES " in
    *" claude-code "*)
      create_symlink "$HOME/.claude/commands/xavier.md" "$SKILL_SOURCE" "$HOME/.claude/commands"
      create_symlink "$HOME/.claude/commands/x.md" "$SKILL_SOURCE" "$HOME/.claude/commands"
      ;;
  esac

  # Cursor and Codex: per-command aliases handle discoverability (installed by install_command_aliases)
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

# --- Helper: remove Xavier-generated aliases from previous prefixes ---
# Scans the three alias roots for per-command aliases whose prefix differs
# from the current ALIAS_PREFIX. Only entries carrying the Xavier marker
# ("xavier router", case-insensitive — present in every template generation)
# are removed, so user-owned files that happen to match the glob (e.g.
# my-review.md) are never touched. Runs against any root that exists,
# regardless of runtime detection — removing our own stale files is safe
# even for runtimes no longer on PATH.
cleanup_stale_aliases() {
  echo "$COMMANDS" | while IFS='|' read -r cmd _desc; do
    [ -z "$cmd" ] && continue

    for stale_file in "$HOME/.claude/commands"/*-"${cmd}.md"; do
      [ -e "$stale_file" ] || continue
      [ "$stale_file" = "$HOME/.claude/commands/${ALIAS_PREFIX}-${cmd}.md" ] && continue
      grep -qi 'xavier router' "$stale_file" 2>/dev/null || continue
      rm "$stale_file"
      info "Removed stale alias: $stale_file"
    done

    for stale_dir in "$HOME/.cursor/skills"/*-"${cmd}" "$HOME/.agents/skills"/*-"${cmd}"; do
      [ -d "$stale_dir" ] || continue
      case "$stale_dir" in
        */"${ALIAS_PREFIX}-${cmd}") continue ;;
      esac
      [ -f "$stale_dir/SKILL.md" ] || continue
      grep -qi 'xavier router' "$stale_dir/SKILL.md" 2>/dev/null || continue
      rm "$stale_dir/SKILL.md"
      if rmdir "$stale_dir" 2>/dev/null; then
        info "Removed stale alias directory: $stale_dir"
      else
        info "Removed stale alias: $stale_dir/SKILL.md (directory not empty, left in place)"
      fi
    done
  done
}

# --- Generate per-command aliases for Claude Code, Cursor, and Codex ---
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
investigate|Investigate a bug or system behavior with structured diagnosis
prd|Create a PRD through user interview, codebase exploration, and module design
tasks|Decompose a PRD into phased implementation tasks
learn|Explore a codebase and produce knowledge notes in the vault
teach|Teach a topic through researched adaptive lessons organized into cohorts
loop|Execute a task file as an autonomous loop using the Shark pattern
mark|Move a PRD or task between active, done, and superseded states
add-dep|Create a dependency-skill for a package with best practices and API patterns
remove-dep|Delete a dependency-skill
research|Research a topic across web, internal docs, and codebase
ask|Answer focused questions about a repo using captured team knowledge
deps-update|Scan lockfile and regenerate stale dependency-skills
export|Export a vault note to your personal Obsidian vault
bug|File a bug report as a GitHub Issue in the Xavier upstream repo
feedback|Open a GitHub Discussion in the Xavier upstream repository
self-update|Update Xavier skills and references to the latest release
uninstall|Remove the Xavier vault and all symlinks
"

  # Remove aliases left behind by previous prefixes before regenerating.
  cleanup_stale_aliases

  echo "$COMMANDS" | while IFS='|' read -r cmd desc; do
    [ -z "$cmd" ] && continue

    # Each runtime's alias is gated on detection — a Claude-only user
    # must not accumulate Cursor/Codex stubs in their skill roots.
    case " $DETECTED_RUNTIMES " in
      *" claude-code "*)
        claude_alias="$HOME/.claude/commands/${ALIAS_PREFIX}-${cmd}.md"
        mkdir -p "$HOME/.claude/commands"
        cat > "$claude_alias" << ALIASEOF
---
name: ${ALIAS_PREFIX}-${cmd}
description: ${desc}
---

This is an alias for \`/xavier ${cmd}\`.

1. Read the Xavier router from \${XAVIER_HOME:-~/.xavier}/SKILL.md (or ~/.xavier/SKILL.md if unset).
2. Follow the Router Lifecycle with subcommand: ${cmd}.
3. Pass through any remaining user arguments unchanged.
4. Stop when the routed ${cmd} command reaches an AskUserQuestion/confirm/wait gate or terminal handoff. Do not infer answers, choose filenames, invoke another Xavier command, or continue into follow-up work unless the user's newest message explicitly asks for it.

Do NOT perform the subcommand's work directly from this alias — load the router first and follow its lifecycle (vault gates, requires resolution, interactive stops).
ALIASEOF
        ;;
    esac

    case " $DETECTED_RUNTIMES " in
      *" cursor "*)
        # Always rewrite so refresh-only and self-update flows pick up
        # content/format changes (e.g. new fields, updated descriptions).
        cursor_alias="$HOME/.cursor/skills/${ALIAS_PREFIX}-${cmd}/SKILL.md"
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
        ;;
    esac

    case " $DETECTED_RUNTIMES " in
      *" codex "*)
        codex_alias="$HOME/.agents/skills/${ALIAS_PREFIX}-${cmd}/SKILL.md"
        mkdir -p "$HOME/.agents/skills/${ALIAS_PREFIX}-${cmd}"
        cat > "$codex_alias" << ALIASEOF
---
name: ${ALIAS_PREFIX}-${cmd}
description: ${desc}. Use when user says /xavier ${cmd}.
---

Route this request through the Xavier router.

1. Read the Xavier router from \${XAVIER_HOME:-~/.xavier}/SKILL.md (or ~/.xavier/SKILL.md if unset).
2. Follow the Router Lifecycle with subcommand: ${cmd}.
3. Pass through any remaining user arguments unchanged.
4. Stop when the routed ${cmd} command reaches an AskUserQuestion/confirm/wait gate or terminal handoff. Do not infer answers, choose filenames, invoke another Xavier command, or continue into follow-up work unless the user's newest message explicitly asks for it.
ALIASEOF
        ;;
    esac
  done

  # Summary message reflects which runtimes actually got aliases written.
  alias_runtimes=""
  case " $DETECTED_RUNTIMES " in *" claude-code "*) alias_runtimes="${alias_runtimes}Claude Code, ";; esac
  case " $DETECTED_RUNTIMES " in *" cursor "*) alias_runtimes="${alias_runtimes}Cursor, ";; esac
  case " $DETECTED_RUNTIMES " in *" codex "*) alias_runtimes="${alias_runtimes}Codex, ";; esac
  alias_runtimes="$(echo "$alias_runtimes" | sed 's/, $//')"

  if [ -n "$alias_runtimes" ]; then
    info "Command aliases installed for $alias_runtimes."
  else
    info "No detected runtimes — skipping command aliases."
  fi
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

    # --- Clean up broken symlinks in ~/.xavier/deps ---
    if [ -d "$XAVIER_HOME/deps" ]; then
      for link in "$XAVIER_HOME/deps/"*; do
        if [ -L "$link" ] && [ ! -e "$link" ]; then
          warn "Removing broken symlink: $link"
          rm "$link"
        fi
      done
    fi

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
          if [ -e "${dep_target}.prev" ]; then
            error "Cannot back up dep: ${dep_name}.prev already exists. Remove it and rerun: rm -r \"${dep_target}.prev\""
            exit 1
          fi
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
  else
    # Existing vault: still ensure new dirs from later releases exist.
    ensure_vault_dirs
  fi

  detect_runtimes
  wire_adapters

  if [ "$EXISTING" = "false" ]; then
    init_git
  fi

  install_skill
  install_command_aliases
  link_xavier_skills_and_refs
  select_native_tool
  print_summary
}

main "$@"
