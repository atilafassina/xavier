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

## Step 3: Strip Prose-Trigger Managed Block

Remove the Xavier-managed block from `~/.claude/CLAUDE.md`. This is the inverse of the block install.sh's `install_prose_trigger()` and self-update's Step 8b write. The byte-level behaviour must match `strip_prose_trigger_block()` in `xavier/install.sh` exactly so a user-edited `CLAUDE.md` survives uninstall byte-for-byte outside the marker region.

The managed block is delimited by these two marker lines (byte-identical to install.sh — `validate-skills.sh` enforces drift detection):

- BEGIN marker: `<!-- BEGIN xavier-prose-trigger -->`
- END marker:   `<!-- END xavier-prose-trigger -->`

### Detection

Let `CLAUDE_MD = $HOME/.claude/CLAUDE.md`.

- **`CLAUDE_MD` does not exist**: silent no-op, proceed to Step 4. Do not create the file.
- **`CLAUDE_MD` exists but does not contain BOTH the BEGIN and END markers**: silent no-op, proceed to Step 4. Never touch a file that does not carry a Xavier-managed block, even if a user added an unbalanced marker themselves.
- **`CLAUDE_MD` exists and contains both markers**: proceed to the strip operation below.

### Strip operation

Read `CLAUDE_MD` and produce a new file whose contents preserve every line outside the marker region exactly, with the following removed:

1. The BEGIN marker line (including its trailing newline).
2. Every line between BEGIN and END.
3. The END marker line (including its trailing newline — this is the "single trailing newline directly following the END marker" that install.sh consumes as part of the line terminator).

Use the LLM's Read/Write tools, or invoke `awk` if running shell. If using awk, mirror `strip_prose_trigger_block()`:

```bash
awk -v begin="<!-- BEGIN xavier-prose-trigger -->" -v end="<!-- END xavier-prose-trigger -->" '
  $0 == begin { skipping = 1; next }
  skipping && $0 == end { skipping = 0; next }
  !skipping { print }
' "$CLAUDE_MD" > "$CLAUDE_MD.tmp"
```

If you use Read/Write directly: read the full bytes of `CLAUDE_MD`, locate the BEGIN marker line, locate the END marker line that follows it, and concatenate the bytes-before-BEGIN with the bytes-after-END-plus-its-newline. Do not add, remove, or modify any other byte. User content above BEGIN and below END must survive byte-for-byte.

### Empty-file cleanup

After stripping, inspect the resulting content. If it is empty or contains only whitespace characters (spaces, tabs, newlines) — meaning Xavier was the sole writer — **remove `CLAUDE_MD` entirely** rather than leave a whitespace-only stub. This mirrors `strip_prose_trigger_block()`'s `tr -d '[:space:]'` residue check.

Otherwise, write the stripped content back to `CLAUDE_MD`.

### Reporting

Track whether the block was removed, whether the file was deleted, or whether the step was a silent no-op. Surface this in the Step 5 summary.

## Step 4: Vault Deletion

Check if `~/.xavier/` exists.

- **If it exists**: Warn the user that this directory contains review history, dependency-skills, knowledge notes, personas, and git history. State clearly that **deletion is permanent and cannot be undone**. Ask for explicit yes/no confirmation using AskUserQuestion.
  - **If the user confirms**: Delete `~/.xavier/` recursively.
  - **If the user declines**: Abort — tell the user symlinks were already removed but the vault is preserved. They can re-run `/xavier setup` to restore symlinks.
- **If it does not exist**: Note that the vault was not found and continue to Step 5.

## Step 5: Summary

Print a final summary listing exactly what was removed and what was not found:

```
Uninstall complete:
  ~/.xavier/                    — removed | not found
  ~/.agents/skills/xavier/      — removed | not found
  ~/.claude/commands/xavier.md  — removed | not found
  ~/.claude/CLAUDE.md block     — stripped | host file deleted | not found
```
