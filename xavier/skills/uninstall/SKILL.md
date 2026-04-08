---
name: uninstall
requires: []
---

# Uninstall

`/xavier uninstall`

Remove the Xavier vault and all registered symlinks. This is a destructive operation.

> **Destructive operation** — follows the [Destructive Operations Protocol](../../references/patterns/destructive-ops.md).

## Step 1: Export Suggestion

Before proceeding, suggest that the user export any valuable data first:

> Your Xavier vault at `~/.xavier/` contains knowledge notes, PRDs, tasks, and review history. Consider backing up or copying any files you want to keep before uninstalling. Would you like to proceed with uninstall? (yes/no)

Use **AskUserQuestion**. If the user declines, abort and report "Uninstall cancelled."

## Step 2: Remove Symlinks

Remove external references **first**, before deleting source data. Check and remove each symlink independently:

1. **`~/.agents/skills/xavier/`** — if it exists (file, symlink, or directory), remove it. If not found, note it.
2. **`~/.claude/commands/xavier.md`** — if it exists (file, symlink, or directory), remove it. If not found, note it.

## Step 3: Vault Deletion

Check if `~/.xavier/` exists.

- **If it exists**: Warn the user that this directory contains review history, dependency-skills, knowledge notes, personas, and git history. State clearly that **deletion is permanent and cannot be undone**. Ask for explicit yes/no confirmation using AskUserQuestion.
  - **If the user confirms**: Delete `~/.xavier/` recursively.
  - **If the user declines**: Abort — tell the user symlinks were already removed but the vault is preserved. They can re-run `/xavier setup` to restore symlinks.
- **If it does not exist**: Note that the vault was not found and continue to Step 4.

## Step 4: Summary

Print a final summary listing exactly what was removed and what was not found:

```
Uninstall complete:
  ~/.xavier/                    — removed | not found
  ~/.agents/skills/xavier/      — removed | not found
  ~/.claude/commands/xavier.md  — removed | not found
```
