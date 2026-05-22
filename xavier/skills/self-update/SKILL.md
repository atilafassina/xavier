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
    # Mirror install.sh's validation regex — reject anything that could let the
    # alias write escape the alias root (`/`, `..`, leading `.`, whitespace, etc.).
    # Fall back to the default `xavier` instead of inheriting bad input.
    if printf '%s' "$prefix_val" | grep -qE '^[a-zA-Z0-9_-]+$'; then
      ALIAS_PREFIX="$prefix_val"
    else
      echo "WARN: Invalid alias-prefix '$prefix_val' in config.md — must be alphanumeric, hyphens, or underscores. Falling back to 'xavier'."
    fi
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

Otherwise, write an alias file for each of the following 19 commands. This human-readable list MUST stay in sync with the executable `COMMANDS` block below and with the `COMMANDS` table in `xavier/install.sh` — adding a skill in one place without the others causes upgrade-vs-fresh-install drift:

| Command | Description |
|---|---|
| setup | Create and configure the Xavier vault |
| review | Run Shark-pattern code review with concurrent reviewer personas |
| babysit | Monitor a PR — poll CI status, auto-fix lint failures, surface review comments |
| grill | Interview about a plan or design until reaching shared understanding |
| investigate | Investigate a bug or system behavior with structured diagnosis |
| prd | Create a PRD through user interview, codebase exploration, and module design |
| tasks | Decompose a PRD into phased implementation tasks |
| learn | Explore a codebase and produce knowledge notes in the vault |
| loop | Execute a task file as an autonomous loop using the Shark pattern |
| mark | Move a PRD or task between active, done, and superseded states |
| add-dep | Create a dependency-skill for a package with best practices and API patterns |
| remove-dep | Delete a dependency-skill |
| research | Research a topic across web, internal docs, and codebase |
| deps-update | Scan lockfile and regenerate stale dependency-skills |
| export | Export a vault note to your personal Obsidian vault |
| bug | File a bug report as a GitHub Issue in the Xavier upstream repo |
| feedback | Open a GitHub Discussion in the Xavier upstream repository |
| self-update | Update Xavier skills and references to the latest release |
| uninstall | Remove the Xavier vault and all symlinks |

First, define the canonical command list as a shell variable. Keep this list in lockstep with the `COMMANDS` table in `xavier/install.sh` — missing entries cause in-product self-update to skip alias regeneration for new skills, leading to drift between fresh installs and updated installs.

```bash
COMMANDS="
setup|Create and configure the Xavier vault
review|Run Shark-pattern code review with concurrent reviewer personas
babysit|Monitor a PR — poll CI status, auto-fix lint failures, surface review comments
grill|Interview about a plan or design until reaching shared understanding
investigate|Investigate a bug or system behavior with structured diagnosis
prd|Create a PRD through user interview, codebase exploration, and module design
tasks|Decompose a PRD into phased implementation tasks
learn|Explore a codebase and produce knowledge notes in the vault
loop|Execute a task file as an autonomous loop using the Shark pattern
mark|Move a PRD or task between active, done, and superseded states
add-dep|Create a dependency-skill for a package with best practices and API patterns
remove-dep|Delete a dependency-skill
research|Research a topic across web, internal docs, and codebase
deps-update|Scan lockfile and regenerate stale dependency-skills
export|Export a vault note to your personal Obsidian vault
bug|File a bug report as a GitHub Issue in the Xavier upstream repo
feedback|Open a GitHub Discussion in the Xavier upstream repository
self-update|Update Xavier skills and references to the latest release
uninstall|Remove the Xavier vault and all symlinks
"
```

Regenerate aliases for **every runtime** that the original install touched, using each runtime's own alias layout. The Claude Code and Cursor formats differ — they are NOT interchangeable — so this step must mirror `install_command_aliases()` in `xavier/install.sh` exactly:

- **Claude Code**: a single Markdown file at `~/.claude/commands/<prefix>-<cmd>.md` containing frontmatter + Skill-tool delegation instructions.
- **Cursor**: a directory at `~/.cursor/skills/<prefix>-<cmd>/` with `SKILL.md` inside, containing frontmatter + a "When user says /xavier <cmd>" trigger description.

Mirror `install_command_aliases()` exactly: when aliases are enabled, regenerate aliases for **both runtimes unconditionally**, creating the root directories if they don't already exist. The original installer does not gate on directory existence — it `mkdir -p`s and writes — so self-update must too. Otherwise users who deleted an alias root (or whose original install never created one) would silently miss alias regeneration on update.

```bash
echo "$COMMANDS" | while IFS='|' read -r cmd desc; do
  [ -z "$cmd" ] && continue

  # Claude Code: single-file alias at ~/.claude/commands/<prefix>-<cmd>.md
  mkdir -p "$HOME/.claude/commands"
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

  # Cursor: directory alias at ~/.cursor/skills/<prefix>-<cmd>/SKILL.md
  mkdir -p "$HOME/.cursor/skills/${ALIAS_PREFIX}-${cmd}"
  cat > "$HOME/.cursor/skills/${ALIAS_PREFIX}-${cmd}/SKILL.md" << ALIASEOF
---
name: ${ALIAS_PREFIX}-${cmd}
description: "${desc}. Use when user says /xavier ${cmd}."
---

Execute /xavier ${cmd}.

1. Read the Xavier router from \${XAVIER_HOME:-~/.xavier}/SKILL.md (or ~/.xavier/SKILL.md if unset)
2. Follow the Router Lifecycle with subcommand: ${cmd}
ALIASEOF
done
```

Once every alias has been regenerated for both runtimes (currently 19 entries × 2 runtimes = 38 alias files), proceed to Step 8b. If `install_command_aliases()` in `xavier/install.sh` ever changes its format, paths, or runtime set, this block must be updated to match.

## Step 8b: Refresh Prose-Trigger Managed Block

After regenerating command aliases, refresh the prose-trigger managed block in `~/.claude/CLAUDE.md` so existing users running `/xavier self-update` pick up the prose-trigger feature without re-running `install.sh`. This step mirrors `install_prose_trigger()` in `xavier/install.sh` (around line 479) — the BEGIN/END marker strings, the block template, and the substitution rules MUST stay byte-identical to what `install_prose_trigger()` writes for the same config inputs. Drift between this step and `install_prose_trigger()` is caught by the marker-drift check in `validate-skills.sh`.

### Read config values

Read `prose-trigger` and `trigger-word` from `$XAVIER_HOME/config.md`:

- **prose-trigger**: default `no`. Lowercase the value before comparing; treat both `yes` and `true` as enabled. Any other value (including missing) means disabled.
- **trigger-word**: default `Xavier`. Must match `^[a-zA-Z][a-zA-Z0-9-]{0,31}$`. If the configured value fails this regex, warn and fall back to `Xavier` — do not inherit invalid input.

Mirror the parsing exactly as `install_prose_trigger()` does:

```bash
PROSE_TRIGGER_ENABLED="no"
TRIGGER_WORD="Xavier"

if [ -f "$XAVIER_HOME/config.md" ]; then
  pt_val="$(grep -o '\*\*prose-trigger\*\*: *[a-zA-Z]*' "$XAVIER_HOME/config.md" 2>/dev/null | head -n 1 | awk -F': *' '{print $2}')"
  pt_val="$(echo "$pt_val" | tr '[:upper:]' '[:lower:]')"
  if [ "$pt_val" = "yes" ] || [ "$pt_val" = "true" ]; then
    PROSE_TRIGGER_ENABLED="yes"
  fi

  tw_val="$(grep -o '\*\*trigger-word\*\*: *[^ ]*' "$XAVIER_HOME/config.md" 2>/dev/null | head -n 1 | awk -F': *' '{print $2}')"
  if [ -n "$tw_val" ]; then
    if printf '%s' "$tw_val" | grep -qE '^[a-zA-Z][a-zA-Z0-9-]{0,31}$'; then
      TRIGGER_WORD="$tw_val"
    else
      echo "WARN: Invalid trigger-word '$tw_val' — must match ^[a-zA-Z][a-zA-Z0-9-]{0,31}$. Falling back to 'Xavier'."
    fi
  fi
fi
```

### Disabled-config strip path

If `PROSE_TRIGGER_ENABLED` is `"no"` (the default), strip any managed block already present in `~/.claude/CLAUDE.md` rather than refresh one. The strip behaviour mirrors `strip_prose_trigger_block()` in `xavier/install.sh` byte-for-byte so that running `install.sh` with `prose-trigger: no` and running `/xavier self-update` with `prose-trigger: no` produce identical post-states for the same `CLAUDE.md` input.

Let `CLAUDE_MD = $HOME/.claude/CLAUDE.md`.

1. **`CLAUDE_MD` does not exist** → silent no-op. Proceed to Step 8c. Do not create the file.
2. **`CLAUDE_MD` exists but does not contain BOTH the BEGIN and END markers** → silent no-op. Proceed to Step 8c. Never touch a host file that does not carry a Xavier-managed block.
3. **`CLAUDE_MD` exists and contains both markers** → strip the marker-delimited region (markers inclusive), then:
   - If the resulting bytes are empty or whitespace-only (Xavier was the sole writer) → **delete** `CLAUDE_MD`. Report "Removed empty ~/.claude/CLAUDE.md after stripping prose-trigger block."
   - Otherwise → write the stripped content back. Report "Stripped prose-trigger block from ~/.claude/CLAUDE.md."

Byte contract for the strip operation — must match `strip_prose_trigger_block()` exactly:

- Remove the BEGIN marker line (and its terminating newline).
- Remove every line between BEGIN and END.
- Remove the END marker line (and its terminating newline — this is the "single trailing newline directly following the END marker").
- Preserve every other byte of `CLAUDE_MD`: leading content, trailing content, blank lines, anything outside the marker region.

Use the LLM's Read/Write tools rather than awk — the install.sh shell script needs awk for portability, but the resulting bytes here must match. After writing (or deleting), proceed to Step 8c. Do NOT execute the enabled-path subcommand-list/marker/template logic below.

### Build the subcommand list

Mirror `install_prose_trigger()`'s comma-join: read the canonical `COMMANDS` variable defined in Step 8a above (the single source of truth shared with `install_command_aliases()` in `xavier/install.sh`) and join the first column with `", "`. Do not introduce a parallel list — the marker-drift check assumes both writers consume the same canonical source.

```bash
SUBCOMMAND_LIST="$(echo "$COMMANDS" | awk -F'|' '
  NF > 0 && $1 != "" {
    if (out == "") { out = $1 } else { out = out ", " $1 }
  }
  END { print out }
')"
```

### Marker strings

These two strings MUST be byte-identical to the ones in `install_prose_trigger()`. `validate-skills.sh` runs a marker-drift check between this SKILL.md and `xavier/install.sh` — changing either one without updating the other causes a build break.

- BEGIN marker: `<!-- BEGIN xavier-prose-trigger -->`
- END marker:   `<!-- END xavier-prose-trigger -->`

### Managed block template

Construct the managed block content with `${TRIGGER_WORD}` and `${SUBCOMMAND_LIST}` substituted. The body below MUST stay byte-identical to the `MANAGEDEOF` heredoc of `install_prose_trigger()` — same line breaks, same Skill-tool-delegation framing.

Source-of-truth lesson: `[[prd/fix-alias-skills]]`.

```
<!-- BEGIN xavier-prose-trigger -->
## Xavier prose trigger

When the user addresses you as "${TRIGGER_WORD}" in vocative position — sentence-initial
"${TRIGGER_WORD}, …", "${TRIGGER_WORD}: …", "Hey ${TRIGGER_WORD} …", "OK ${TRIGGER_WORD} …" —
treat it as a Xavier invocation. Mid-sentence "${TRIGGER_WORD}" or lowercase variants do NOT
trigger.

Routing:
- Subcommand keyword present (${SUBCOMMAND_LIST}): Use the Skill tool to invoke:
  - skill: "xavier"
  - args: "<cmd> <remaining-prose>"
  Do NOT execute the skill directly. Do NOT read vault files. Delegate to the xavier router.
- No keyword, intent clear: confirm with one line — "Sounds like a grill.
  Run /xavier grill? (Y/n)" — then on assent, Use the Skill tool to invoke:
  - skill: "xavier"
  - args: "<inferred-cmd> <remaining-prose>"
- Meta question about Xavier ("what can you do?", "help", "list commands"):
  Use the Skill tool to invoke:
  - skill: "xavier"
  - args: ""
  The router lists subcommands.
- Off-topic or no confident subcommand match: drop the trigger and answer
  normally without invoking a skill.
<!-- END xavier-prose-trigger -->
```

### Write logic — three idempotent cases

Mirror the three branches of `install_prose_trigger()`'s file write. Use the LLM's Read/Write/Edit tools rather than awk+getline — the install.sh shell script needs awk for portability, but here we have direct file tools. The resulting file content MUST be byte-identical to what `install_prose_trigger()` produces for the same config inputs.

Let `CLAUDE_MD = $HOME/.claude/CLAUDE.md`.

1. **Create-from-empty** — `CLAUDE_MD` does not exist:
   - Ensure `$HOME/.claude/` exists (`mkdir -p`).
   - Write `CLAUDE_MD` with the managed block as its entire contents. Include the trailing newline after the END marker that the install heredoc emits (the block ends with `<!-- END xavier-prose-trigger -->\n`).
   - Report "Created ~/.claude/CLAUDE.md with prose-trigger block."

2. **Replace-between-markers** — `CLAUDE_MD` exists AND both BEGIN and END markers are present:
   - Read `CLAUDE_MD`.
   - Replace the region from the BEGIN marker line through the END marker line (inclusive on both sides) with the freshly substituted managed block (the same `<!-- BEGIN ... -->` line, body, and `<!-- END ... -->` line).
   - Preserve every byte of the file outside that region — leading content, trailing content, blank lines, anything. The user's own notes in `CLAUDE.md` must survive untouched.
   - Write `CLAUDE_MD` back.
   - Report "Refreshed prose-trigger block in ~/.claude/CLAUDE.md."

3. **Append-with-markers** — `CLAUDE_MD` exists BUT either marker is absent:
   - Read `CLAUDE_MD`.
   - Append a single blank line, then the managed block (BEGIN marker, body, END marker, trailing newline). The blank-line separator matches what `install_prose_trigger()`'s `printf '\n' >> "$CLAUDE_MD"` produces.
   - Write `CLAUDE_MD` back.
   - Report "Appended prose-trigger block to ~/.claude/CLAUDE.md."

The three cases are mutually exclusive and exhaustive — do not skip the detection step. Running `/xavier self-update` twice in succession MUST produce exactly one managed block (case 2 on the second run replaces in place). This is PRD smoke scenario 6.

Then proceed to Step 8c.

## Step 8c: Refresh Cursor Prose-Trigger Skill

After Step 8b (Claude `CLAUDE.md` block), refresh the Cursor prose-trigger skill at `~/.cursor/skills/prose-trigger/SKILL.md`. This step mirrors `install_cursor_prose_trigger_skill()` in `xavier/install.sh` — the skill template and substitution rules MUST stay byte-identical to what `install.sh` writes for the same config inputs. Drift between this step and `install.sh` is caught by the Cursor template-drift check in `validate-skills.sh`.

The skill name is **fixed** as `prose-trigger` (not `${ALIAS_PREFIX}-prose-trigger`) so it does not appear in prefix-filtered slash autocomplete (`/x-`, `/xavier-`).

### Reuse config from Step 8b

Use the same `PROSE_TRIGGER_ENABLED`, `TRIGGER_WORD`, and `SUBCOMMAND_LIST` values already parsed in Step 8b. Do not re-read config unless Step 8b was skipped.

### Disabled-config strip path

If `PROSE_TRIGGER_ENABLED` is `"no"`, remove `~/.cursor/skills/prose-trigger/` if it exists (`rm -rf`). Silent no-op when absent. Report "Removed Cursor prose-trigger skill" or silent no-op. Then proceed to Step 9.

### Enabled path — skill template

Write (overwrite) `~/.cursor/skills/prose-trigger/SKILL.md` with the content below, substituting `${TRIGGER_WORD}` and `${SUBCOMMAND_LIST}`. The body MUST stay byte-identical to the `CURSORPROSEEOF` heredoc in `install_cursor_prose_trigger_skill()` inside `xavier/install.sh`.

```
---
name: prose-trigger
description: "Route Xavier prose invocations when user addresses ${TRIGGER_WORD} in vocative position (${TRIGGER_WORD}, … / ${TRIGGER_WORD}: … / Hey ${TRIGGER_WORD} …). Takes precedence over slash-command aliases when the trigger word appears in natural prose. Do NOT suggest /prose-trigger — this skill is for natural-language vocative routing only."
---

# Xavier prose trigger

When the user addresses you as "${TRIGGER_WORD}" in vocative position — sentence-initial
"${TRIGGER_WORD}, …", "${TRIGGER_WORD}: …", "Hey ${TRIGGER_WORD} …", "OK ${TRIGGER_WORD} …" —
treat it as a Xavier invocation. Mid-sentence "${TRIGGER_WORD}" or lowercase variants do NOT
trigger.

When the trigger word appears in vocative form, this skill takes precedence over slash-command
aliases (e.g. /x-grill) — route through the Xavier router below, not an alias skill.

Routing:
- Subcommand keyword present (${SUBCOMMAND_LIST}):
  1. Read the Xavier router from ${XAVIER_HOME:-~/.xavier}/SKILL.md (or ~/.xavier/SKILL.md if unset)
  2. Follow the Router Lifecycle with subcommand: <cmd>
  Do NOT execute vault skills directly. Delegate to the xavier router.
- No keyword, intent clear: confirm with one line — "Sounds like a grill.
  Run /xavier grill? (Y/n)" — then on assent:
  1. Read the Xavier router from ${XAVIER_HOME:-~/.xavier}/SKILL.md (or ~/.xavier/SKILL.md if unset)
  2. Follow the Router Lifecycle with subcommand: <inferred-cmd>
- Meta question about Xavier ("what can you do?", "help", "list commands"):
  1. Read the Xavier router from ${XAVIER_HOME:-~/.xavier}/SKILL.md (or ~/.xavier/SKILL.md if unset)
  2. Follow the Router Lifecycle with no subcommand (router lists subcommands)
- Off-topic or no confident subcommand match: drop the trigger and answer
  normally without invoking the router.
```

**Do not set `disable-model-invocation: true`.** Omit the field so the agent can attach this skill on vocative prose.

### Write logic

1. Ensure `~/.cursor/skills/prose-trigger/` exists (`mkdir -p`).
2. Write `SKILL.md` with the substituted template above (full overwrite — idempotent).
3. Report "Installed Cursor prose-trigger skill at ~/.cursor/skills/prose-trigger/SKILL.md."

Running `/xavier self-update` twice MUST leave exactly one skill directory with one `SKILL.md`.

## Step 9: Update Version in Config

Find the line containing `**version**:` in `$XAVIER_HOME/config.md` and update the value to the new version.

- If the line exists, replace it: `- **version**: {new_version}`
- If no version field exists, add `- **version**: {new_version}` under the `## Preferences` section.

Do NOT modify any other content in `config.md`.

## Step 10: Ensure Vault Directories

Create any new vault directories that new skills might expect. The set below MUST mirror `ensure_vault_dirs()` in `xavier/install.sh` line-for-line so in-product self-update produces the exact layout of a fresh install (including lifecycle archive subdirs and any partially-pruned legacy dirs):

```bash
mkdir -p "$XAVIER_HOME/personas"
mkdir -p "$XAVIER_HOME/adapters"
mkdir -p "$XAVIER_HOME/skills"
mkdir -p "$XAVIER_HOME/deps"
mkdir -p "$XAVIER_HOME/knowledge/repos"
mkdir -p "$XAVIER_HOME/knowledge/teams"
mkdir -p "$XAVIER_HOME/knowledge/reviews"
mkdir -p "$XAVIER_HOME/prd"
mkdir -p "$XAVIER_HOME/prd/done"
mkdir -p "$XAVIER_HOME/tasks"
mkdir -p "$XAVIER_HOME/tasks/done"
mkdir -p "$XAVIER_HOME/research"
mkdir -p "$XAVIER_HOME/investigations"
mkdir -p "$XAVIER_HOME/loop-state"
mkdir -p "$XAVIER_HOME/shark-state"
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
