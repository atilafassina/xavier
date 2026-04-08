---
name: uninstall
requires: []
---

# Uninstall

`/xavier uninstall`

Remove the Xavier vault and all registered symlinks. This is a destructive operation.

## Step 1: Vault Deletion

Check if `~/.xavier/` exists.

- **If it exists**: Warn the user that this directory contains review history, dependency-skills, knowledge notes, personas, and git history. State clearly that **deletion is permanent and cannot be undone**. Ask for explicit yes/no confirmation using AskUserQuestion.
  - **If the user confirms**: Delete `~/.xavier/` recursively.
  - **If the user declines**: Abort the entire uninstall — do NOT proceed to symlink removal. Tell the user nothing was changed.
- **If it does not exist**: Note that the vault was not found and continue to Step 2.

## Step 2: Remove Symlinks

Check and remove each symlink independently. For each one:

1. **`~/.agents/skills/xavier/`** — if it exists (file, symlink, or directory), remove it. If not found, note it.
2. **`~/.claude/commands/xavier.md`** — if it exists (file, symlink, or directory), remove it. If not found, note it.

## Step 3: Summary

Print a final summary listing exactly what was removed and what was not found:

```
Uninstall complete:
  ~/.xavier/                    — removed | not found
  ~/.agents/skills/xavier/      — removed | not found
  ~/.claude/commands/xavier.md  — removed | not found
```
