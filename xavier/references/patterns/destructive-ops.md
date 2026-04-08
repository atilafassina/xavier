# Destructive Operations Protocol

A shared safeguard contract for any skill that deletes, overwrites, or irreversibly modifies user data. Skills performing destructive operations MUST follow these four steps in order.

## 1. Confirm

Before any irreversible action, use **AskUserQuestion** to get explicit yes/no confirmation. The prompt must clearly state:

- What will be destroyed or overwritten
- That the action cannot be undone (or how to undo it)

Never proceed on silence or ambiguity — only on explicit "yes."

## 2. Commit Before

Create a checkpoint commit so the user can recover:

```bash
git add -A && git commit -m "pre-<operation> checkpoint"
```

If the working tree is clean (nothing to commit), skip the commit but log that no checkpoint was needed.

This step applies when operating inside a git repository. If outside a repo, skip it and note that no checkpoint was created.

## 3. Order of Deletion

When removing something that has both **source data** and **external references** (symlinks, config entries, index entries):

1. Remove external references first (symlinks, config pointers, index entries)
2. Delete source data last

This ensures no dangling references remain if the operation is interrupted.

## 4. Report

After the operation completes, tell the user:

- Exactly what was removed/changed
- How to undo it (e.g., `git checkout HEAD~1 -- <path>`, or "re-run `/xavier setup`")

If a checkpoint commit was created in step 2, mention it explicitly so the user knows they can revert.
