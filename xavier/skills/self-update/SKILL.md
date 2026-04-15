---
name: self-update
requires: [config, deps-index:optional]
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

> Update Xavier from v{current} to v{target}? This will replace skills/, references/, and distributed deps/ in your vault. Your knowledge, config, memory, PRDs, tasks, and user-created deps are preserved. (yes/no)

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

Verify that `$TMPDIR/xavier/skills/` and `$TMPDIR/xavier/references/` exist after extraction. If not, report an error, clean up, and stop. The `$TMPDIR/xavier/deps/` directory is optional — if absent, skip dep updates in Step 8.

## Step 8: Replace Distributable Files

Overwrite distributable files in the vault (`$XAVIER_HOME`). Only replace the following directories — nothing else.

**Back up before replacing** so a partial failure can be rolled back:

```bash
# Create backup of current distributable directories
cp -R "$XAVIER_HOME/skills/" "$TMPDIR/skills-backup/"
cp -R "$XAVIER_HOME/references/" "$TMPDIR/references-backup/"
[ -f "$XAVIER_HOME/SKILL.md" ] && cp "$XAVIER_HOME/SKILL.md" "$TMPDIR/SKILL-backup.md"

# Back up only the distributed deps that will be replaced (not user-created ones)
if [ -d "$TMPDIR/xavier/deps" ]; then
  mkdir -p "$TMPDIR/deps-backup"
  for dep_dir in "$TMPDIR/xavier/deps/"*/; do
    [ -d "$dep_dir" ] || continue
    dep_name="$(basename "$dep_dir")"
    [ -d "$XAVIER_HOME/deps/$dep_name" ] && cp -R "$XAVIER_HOME/deps/$dep_name" "$TMPDIR/deps-backup/$dep_name"
  done
fi
```

Run this **entire block as a single Bash command** — do not split it or skip any section:

```bash
# 1. Replace skills and references
rm -rf "$XAVIER_HOME/skills/" "$XAVIER_HOME/references/"
cp -R "$TMPDIR/xavier/skills/" "$XAVIER_HOME/skills/"
cp -R "$TMPDIR/xavier/references/" "$XAVIER_HOME/references/"

# 2. Merge distributed deps (replace tarball deps, preserve user-created ones)
if [ -d "$TMPDIR/xavier/deps" ]; then
  mkdir -p "$XAVIER_HOME/deps"
  for dep_dir in "$TMPDIR/xavier/deps/"*/; do
    [ -d "$dep_dir" ] || continue
    dep_name="$(basename "$dep_dir")"
    rm -rf "$XAVIER_HOME/deps/$dep_name"
    cp -R "$dep_dir" "$XAVIER_HOME/deps/$dep_name"
  done
fi

# 3. Update router
[ -f "$TMPDIR/xavier/SKILL.md" ] && cp "$TMPDIR/xavier/SKILL.md" "$XAVIER_HOME/SKILL.md"

echo "Replaced: skills/, references/, distributed deps, SKILL.md"
```

**Rollback on partial failure**: If any copy command above fails, restore from backup immediately:

```bash
# Rollback — restore previous versions
rm -rf "$XAVIER_HOME/skills/" "$XAVIER_HOME/references/"
cp -R "$TMPDIR/skills-backup/" "$XAVIER_HOME/skills/"
cp -R "$TMPDIR/references-backup/" "$XAVIER_HOME/references/"
[ -f "$TMPDIR/SKILL-backup.md" ] && cp "$TMPDIR/SKILL-backup.md" "$XAVIER_HOME/SKILL.md"

# Rollback distributed deps
if [ -d "$TMPDIR/deps-backup" ]; then
  for dep_dir in "$TMPDIR/deps-backup/"*/; do
    [ -d "$dep_dir" ] || continue
    dep_name="$(basename "$dep_dir")"
    rm -rf "$XAVIER_HOME/deps/$dep_name"
    cp -R "$dep_dir" "$XAVIER_HOME/deps/$dep_name"
  done
fi
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
- `babysit-pr/`
- `.obsidian/`

**Note on `deps/`**: Distributed deps (those present in the release tarball) are replaced during update. User-created deps (added via `/xavier add-dep`) are preserved — only dep directories that exist in the tarball are overwritten.

## Step 8a: Regenerate Command Aliases

After replacing distributable files, regenerate the Claude Code command alias files so they stay in sync with the updated skill set. These alias files live at `~/.claude/commands/<prefix>-<cmd>.md` and allow users to invoke Xavier subcommands directly (e.g., `/xavier-review`).

First, read the alias prefix and check whether aliases are enabled:

```bash
# Read alias prefix from config (default: xavier)
ALIAS_PREFIX="xavier"
if [ -f "$XAVIER_HOME/config.md" ]; then
  prefix_val="$(grep -o '\*\*alias-prefix\*\*: *[^ ]*' "$XAVIER_HOME/config.md" 2>/dev/null | head -n 1 | awk -F': *' '{print $2}')"
  if [ -n "$prefix_val" ]; then
    ALIAS_PREFIX="$prefix_val"
  fi
fi

# Check if command aliases are disabled
ALIASES_ENABLED="yes"
if [ -f "$XAVIER_HOME/config.md" ]; then
  config_val="$(grep -o '\*\*command-aliases\*\*: *[a-zA-Z]*' "$XAVIER_HOME/config.md" 2>/dev/null | head -n 1 | awk -F': *' '{print $2}')"
  config_val="$(echo "$config_val" | tr '[:upper:]' '[:lower:]')"
  if [ "$config_val" = "no" ] || [ "$config_val" = "false" ]; then
    ALIASES_ENABLED="no"
  fi
fi
```

If `ALIASES_ENABLED` is `"no"`, skip the rest of this step and proceed to Step 9.

Otherwise, write an alias file for each of the following 15 commands:

| Command | Description |
|---|---|
| setup | Create and configure the Xavier vault |
| review | Run Shark-pattern code review with concurrent reviewer personas |
| babysit | Monitor a PR — poll CI status, auto-fix lint failures, surface review comments |
| grill | Interview about a plan or design until reaching shared understanding |
| prd | Create a PRD through user interview, codebase exploration, and module design |
| tasks | Decompose a PRD into phased implementation tasks |
| learn | Explore a codebase and produce knowledge notes in the vault |
| loop | Execute a task file as an autonomous loop using the Shark pattern |
| add-dep | Create a dependency-skill for a package with best practices and API patterns |
| remove-dep | Delete a dependency-skill |
| research | Research a topic across web, internal docs, and codebase |
| deps-update | Scan lockfile and regenerate stale dependency-skills |
| export | Export a vault note to your personal Obsidian vault |
| self-update | Update Xavier skills and references to the latest release |
| uninstall | Remove the Xavier vault and all symlinks |

For each command, write the alias file at `~/.claude/commands/${ALIAS_PREFIX}-${cmd}.md` with the following content:

```bash
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
research|Research a topic across web, internal docs, and codebase
deps-update|Scan lockfile and regenerate stale dependency-skills
export|Export a vault note to your personal Obsidian vault
self-update|Update Xavier skills and references to the latest release
uninstall|Remove the Xavier vault and all symlinks
"

mkdir -p "$HOME/.claude/commands"

echo "$COMMANDS" | while IFS='|' read -r cmd desc; do
  [ -z "$cmd" ] && continue
  cat > "$HOME/.claude/commands/${ALIAS_PREFIX}-${cmd}.md" << ALIASEOF
---
name: ${ALIAS_PREFIX}-${cmd}
description: ${desc}
---

This is an alias for \`/xavier ${cmd}\`.

Use the Skill tool to invoke:
- skill: "xavier"
- args: "${cmd}" followed by any arguments provided by the user

Do NOT execute this skill directly. Do NOT read vault files. Delegate to the xavier router.
ALIASEOF
done
```

Once all 15 alias files have been written, proceed to Step 9.

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
mkdir -p "$XAVIER_HOME/loop-state"
mkdir -p "$XAVIER_HOME/shark-state"
mkdir -p "$XAVIER_HOME/deps"
mkdir -p "$XAVIER_HOME/research"
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
