#!/bin/sh
# Xavier Uninstaller
# Reverses everything install.sh creates.
# Usage: sh /path/to/xavier/uninstall.sh

set -eu

XAVIER_HOME="${XAVIER_HOME:-$HOME/.xavier}"

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

REMOVED=""
SKIPPED=""

track_removed() { REMOVED="${REMOVED}  - $1\n"; }
track_skipped() { SKIPPED="${SKIPPED}  - $1\n"; }

# --- Remove a symlink if it exists ---
remove_link() {
  target="$1"
  if [ -L "$target" ]; then
    rm "$target"
    info "Removed symlink: $target"
    track_removed "$target"
  elif [ -e "$target" ]; then
    warn "Not a symlink, leaving alone: $target"
    track_skipped "$target (not a symlink)"
  else
    track_skipped "$target (not found)"
  fi
}

# --- Main ---
bold "Xavier Uninstaller"
echo ""

# 1. Remove ~/.agents/skills/xavier symlink
remove_link "$HOME/.agents/skills/xavier"

# 2. Remove ~/.claude/commands/xavier.md symlink
remove_link "$HOME/.claude/commands/xavier.md"

# 3. Remove per-skill symlinks in $XAVIER_HOME/skills/
if [ -d "$XAVIER_HOME/skills" ]; then
  for link in "$XAVIER_HOME/skills/"*; do
    # Guard against empty glob
    [ -e "$link" ] || [ -L "$link" ] || continue
    if [ -L "$link" ]; then
      rm "$link"
      info "Removed skill symlink: $link"
      track_removed "$link"
    fi
  done
else
  track_skipped "$XAVIER_HOME/skills/ (directory not found)"
fi

# 4. Remove references symlink in $XAVIER_HOME/references
remove_link "$XAVIER_HOME/references"

# 5. Prompt before deleting the vault directory
echo ""
if [ -d "$XAVIER_HOME" ]; then
  warn "The Xavier vault directory still exists at: $XAVIER_HOME"
  echo ""
  echo "  This directory contains your personalized configuration,"
  echo "  memory, knowledge base, PRDs, tasks, and review state."
  echo "  Deleting it will permanently remove all of this data."
  echo ""
  printf "  Delete vault directory %s? [y/N]: " "$XAVIER_HOME"
  read -r confirm
  case "$confirm" in
    y|Y|yes|YES)
      rm -rf "$XAVIER_HOME"
      info "Deleted vault directory: $XAVIER_HOME"
      track_removed "$XAVIER_HOME (vault directory)"
      ;;
    *)
      info "Kept vault directory: $XAVIER_HOME"
      track_skipped "$XAVIER_HOME (kept by user)"
      ;;
  esac
else
  track_skipped "$XAVIER_HOME (vault directory not found)"
fi

# --- Summary ---
echo ""
bold "Uninstall summary"
echo ""
if [ -n "$REMOVED" ]; then
  info "Removed:"
  printf "$REMOVED"
fi
if [ -n "$SKIPPED" ]; then
  info "Skipped:"
  printf "$SKIPPED"
fi
echo ""
info "Xavier has been uninstalled."
