---
name: remove-dep
requires: [config, deps-index]
---

# Remove Dependency Skill

`/xavier remove-dep <package-name>`

> **Destructive operation** — follows the [Destructive Operations Protocol](../../references/patterns/destructive-ops.md).

## Step 1: Validate

1. Check that `<package-name>` was provided. If not, ask the user.
2. Check if `~/.xavier/deps/<package-name>/` exists (using the resolved `deps-index` context). If not, tell the user it doesn't exist and list available dependency-skills.
3. **Scope guard**: Verify that the target path resolves to a directory inside `~/.xavier/deps/` and NOT inside `~/.xavier/skills/` or any other vault directory. If the resolved path escapes `deps/`, refuse the operation and tell the user why.

## Step 2: Confirm

Use **AskUserQuestion** to get explicit confirmation before deletion:

> About to permanently remove dependency-skill **{package-name}** (`~/.xavier/deps/{package-name}/`). This cannot be undone. Proceed? (yes/no)

If the user declines, abort and report "Removal cancelled."

## Step 3: Checkpoint

Create a pre-deletion checkpoint commit in the vault:

```bash
cd ~/.xavier && git add -A && git commit -m "pre-remove-dep checkpoint ({package-name})"
```

If the working tree is clean, skip the commit but note it.

## Step 4: Remove

Delete the directory `~/.xavier/deps/<package-name>/`.

## Step 5: Report

Tell the user the dependency-skill was removed. Mention the checkpoint commit so they know they can revert with:

```
cd ~/.xavier && git checkout HEAD~1 -- deps/{package-name}
```
