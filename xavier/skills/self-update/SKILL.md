---
name: self-update
requires: [config]
---

# Self-Update

`/xavier self-update [version]`

Update Xavier's skills, references, and router to the latest release (or a specific version) from GitHub.

## Step 1: Determine Current Version

Read the resolved `config.md` (already loaded via the `config` require). Find the line matching `**version**:` and extract the current version string. If no version field exists, treat the current version as `0.0.0`.

## Step 2: Determine Target Version

Check if the user passed a version argument (e.g., `/xavier self-update v0.3.0`).

- **If a version argument was provided**: use that as the target. Strip the `v` prefix if present for comparison purposes.
- **If no version argument was provided**: fetch the latest release tag from GitHub.

  Try `gh` first, fall back to `curl`:

  ```bash
  gh api repos/atilafassina/xavier/releases/latest --jq '.tag_name' 2>/dev/null | sed 's/^v//'
  ```

  If `gh` fails or is not available:

  ```bash
  curl -fsSL https://api.github.com/repos/atilafassina/xavier/releases/latest | grep '"tag_name"' | sed 's/.*"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/'
  ```

## Step 3: Compare Versions

Compare the current installed version against the target version (both without `v` prefix).

- **If they are equal**: report "Already up to date (v{version})" and **STOP** — do not proceed further.
- **If they differ**: continue to Step 4.

## Step 4: Display Update Summary

Show the user:

```
Xavier update available: v{current} → v{target}
```

If release notes are available from the GitHub release body, fetch them and show a brief summary (2-3 sentences max). Use:

```bash
gh api repos/atilafassina/xavier/releases/latest --jq '.body' 2>/dev/null
```

Or via `curl` if `gh` is unavailable.

## Step 5: Confirm with User

Ask the user to confirm before proceeding. Use AskUserQuestion:

> Update Xavier from v{current} to v{target}? This will replace skills/ and references/ in your vault. Your knowledge, config, memory, PRDs, and tasks are preserved. (yes/no)

If the user declines, abort and report "Update cancelled."

## Step 6: Download Release

Download the release tarball to a temporary directory:

```bash
TMPDIR=$(mktemp -d)
curl -fsSL "https://github.com/atilafassina/xavier/releases/download/v${TARGET_VERSION}/xavier.tar.gz" -o "$TMPDIR/xavier.tar.gz"
```

If the download fails, report the error and clean up the temp directory. Do not proceed.

## Step 7: Extract Tarball

Extract the tarball in the temp directory:

```bash
tar -xzf "$TMPDIR/xavier.tar.gz" -C "$TMPDIR"
```

Verify that `$TMPDIR/xavier/skills/` and `$TMPDIR/xavier/references/` exist after extraction. If not, report an error, clean up, and stop.

## Step 8: Replace Distributable Files

Overwrite distributable files in the vault (`$XAVIER_HOME`). Only replace the following directories — nothing else.

**Back up before replacing** so a partial failure can be rolled back:

```bash
# Create backup of current distributable directories
cp -R "$XAVIER_HOME/skills/" "$TMPDIR/skills-backup/"
cp -R "$XAVIER_HOME/references/" "$TMPDIR/references-backup/"
[ -f "$XAVIER_HOME/SKILL.md" ] && cp "$XAVIER_HOME/SKILL.md" "$TMPDIR/SKILL-backup.md"
```

```bash
# Remove old distributable directories
rm -rf "$XAVIER_HOME/skills/"
rm -rf "$XAVIER_HOME/references/"

# Copy new distributable directories from tarball
cp -R "$TMPDIR/xavier/skills/" "$XAVIER_HOME/skills/"
cp -R "$TMPDIR/xavier/references/" "$XAVIER_HOME/references/"
```

If the tarball contains `xavier/SKILL.md` (the router), copy it to the appropriate location:

```bash
cp "$TMPDIR/xavier/SKILL.md" "$XAVIER_HOME/SKILL.md"
```

**Rollback on partial failure**: If any copy command above fails, restore from backup immediately:

```bash
# Rollback — restore previous versions
rm -rf "$XAVIER_HOME/skills/" "$XAVIER_HOME/references/"
cp -R "$TMPDIR/skills-backup/" "$XAVIER_HOME/skills/"
cp -R "$TMPDIR/references-backup/" "$XAVIER_HOME/references/"
[ -f "$TMPDIR/SKILL-backup.md" ] && cp "$TMPDIR/SKILL-backup.md" "$XAVIER_HOME/SKILL.md"
```

Report the failure to the user, clean up `$TMPDIR`, and **stop** — do not proceed to version update.

### Files and directories that MUST NOT be touched:

- `knowledge/`
- `MEMORY.md`
- `config.md` (except the version field in Step 9)
- `prd/`
- `tasks/`
- `loop-state/`
- `shark-state/`
- `review-state/`
- `deps/`
- `babysit-pr/`
- `.obsidian/`

## Step 9: Update Version in Config

Find the line containing `**version**:` in `$XAVIER_HOME/config.md` and update the value to the new version.

- If the line exists, replace it: `- **version**: {new_version}`
- If no version field exists, add `- **version**: {new_version}` under the `## Preferences` section.

Do NOT modify any other content in `config.md`.

## Step 10: Ensure Vault Directories

Create any new vault directories that new skills might expect. Run `mkdir -p` for the standard set:

```bash
mkdir -p "$XAVIER_HOME/knowledge/repos"
mkdir -p "$XAVIER_HOME/knowledge/teams"
mkdir -p "$XAVIER_HOME/knowledge/reviews"
mkdir -p "$XAVIER_HOME/prd"
mkdir -p "$XAVIER_HOME/tasks"
mkdir -p "$XAVIER_HOME/review-state"
mkdir -p "$XAVIER_HOME/loop-state"
mkdir -p "$XAVIER_HOME/shark-state"
mkdir -p "$XAVIER_HOME/deps"
mkdir -p "$XAVIER_HOME/babysit-pr"
```

## Step 11: Clean Up

Remove the temporary directory:

```bash
rm -rf "$TMPDIR"
```

## Step 12: Report Success

Tell the user:

```
Updated Xavier: v{old} → v{new}
```

List any new skills that were added (directories present in the new `skills/` that were not in the old one, if that information is available) or simply confirm that skills and references have been refreshed.
