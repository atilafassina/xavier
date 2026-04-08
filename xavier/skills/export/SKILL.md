---
name: export
requires: [config, prd-index, tasks-index, repo-conventions:optional, team-conventions:optional, recurring-patterns:optional]
---

# Export

`/xavier export [path]`

Export a vault note to the user's personal Obsidian vault with wikilink adaptation and `x-` namespace prefix.

## Step 1: Check Export Config

1. From the resolved `config` context, look for the `## Export` section with `export-vault-path`.
2. **If `export-vault-path` is not configured**: ask the user for their Obsidian vault path using AskUserQuestion. Offer to save it to config (append `## Export` section). If they decline, use the provided path for this export only.
3. Read `export-show-diff` (default: `false`).

## Step 2: Select Note to Export

- **If a path argument was provided** (e.g., `/xavier export prd/my-feature`): resolve it relative to `~/.xavier/`. Verify the file exists.
- **If no argument**: list exportable directories from `~/.xavier/` — show `prd/`, `tasks/`, `knowledge/repos/`, `knowledge/teams/`, `knowledge/reviews/`. **Exclude** internal directories: `personas/`, `adapters/`, `loop-state/`, `skills/`. Present files as a numbered list using AskUserQuestion for the user to pick.

## Step 3: Adapt Wikilinks

Read the selected note's content. Find all wikilinks (`[[...]]`) and rewrite them:

1. **Scan the export destination** (`{export-vault-path}/x-inbox/`) for previously exported files to build an index of exported note names.
2. For each wikilink in the source note:
   - **If the linked note has been exported** (exists as `x-inbox/x-<source-dir>-<name>.md`): rewrite to `[[x-inbox/x-<source-dir>-<name>]]`
   - **If the linked note has NOT been exported**: strip the wikilink brackets and leave the display text as plain text (e.g., `[[my-note|My Note]]` becomes `My Note`, `[[my-note]]` becomes `my-note`)
3. Preserve all other Obsidian-flavored markdown: callouts (`> [!note]`), tags (`#tag`), frontmatter, embedded images, and code blocks.

## Step 4: Write Exported File

1. Create the `x-inbox/` directory inside `{export-vault-path}` if it doesn't exist.
2. The destination path is `{export-vault-path}/x-inbox/x-<source-dir>-<filename>.md` where `<source-dir>` is the immediate parent directory name and `<filename>` is the original filename without path (e.g., `prd/my-feature.md` → `x-prd-my-feature.md`, `knowledge/repos/xavier/architecture.md` → `x-xavier-architecture.md`). Including the source directory prevents collisions when exporting files with the same name from different directories.
3. **If the destination file already exists**:
   - If `export-show-diff: true`: show a diff between existing and new content
   - Ask the user to confirm overwrite using AskUserQuestion (options: Overwrite, Skip)
   - If they skip, abort this export
4. Write the adapted content to the destination.

Tell the user the note was exported and where to find it.
