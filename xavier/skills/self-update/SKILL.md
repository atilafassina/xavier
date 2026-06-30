---
name: self-update
description: Update Xavier's skills, references, router, and native tool binary to the latest release (or a specific version) from GitHub.
requires: [config, deps-index:optional]
---

# Self-Update

`/xavier self-update [version]`

Update Xavier's skills, references, router, and native tool binary to the latest release (or a specific version) from GitHub.

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

# 4. Install the native tool binary for this host triple. Mirrors install.sh's
#    detect_host_triple/select_native_tool, with two safety properties:
#      - the existing binary is moved aside BEFORE replacement and restored if
#        the copy fails, so a failed write is surfaced (stderr), never masked;
#      - when this release ships NO binary for the host triple, any prior-version
#        binary is cleared (the backup is not restored) so merge.sh falls back to
#        parse.sh instead of running a stale binary from an earlier version.
os="$(uname -s 2>/dev/null || echo unknown)"
arch="$(uname -m 2>/dev/null || echo unknown)"
case "$arch" in
  x86_64|amd64)  rust_arch="x86_64" ;;
  arm64|aarch64) rust_arch="aarch64" ;;
  *)             rust_arch="" ;;
esac
case "$os" in
  Darwin) [ -n "$rust_arch" ] && host_triple="${rust_arch}-apple-darwin" || host_triple="" ;;
  Linux)  [ -n "$rust_arch" ] && host_triple="${rust_arch}-unknown-linux-gnu" || host_triple="" ;;
  *)      host_triple="" ;;
esac
if [ -n "$host_triple" ]; then
  dest="$XAVIER_HOME/bin/$host_triple/xavier-tool"
  # Back up any existing binary (mv aside) so a failed copy can be rolled back.
  [ -f "$dest" ] && mv "$dest" "$TMPDIR/bin-backup-xavier-tool"
  if [ -f "$TMPDIR/xavier/bin/$host_triple/xavier-tool" ]; then
    mkdir -p "$XAVIER_HOME/bin/$host_triple"
    if cp "$TMPDIR/xavier/bin/$host_triple/xavier-tool" "$dest"; then
      chmod +x "$dest"
      echo "Installed native tool: bin/$host_triple/xavier-tool"
    else
      # Copy failed: restore the prior binary (if any) and surface the failure.
      [ -f "$TMPDIR/bin-backup-xavier-tool" ] && { mv "$TMPDIR/bin-backup-xavier-tool" "$dest"; chmod +x "$dest"; }
      echo "ERROR: native tool copy failed; restored prior binary if present — merge.sh uses it or falls back to parse.sh" >&2
    fi
  else
    # No bundled binary for this host this release: leave the old one cleared (do
    # NOT restore the backup) so merge.sh falls back to parse.sh, never a stale one.
    echo "No bundled native tool for $host_triple this release — cleared any prior binary; merge.sh falls back to parse.sh"
  fi
fi

echo "Replaced: skills/, references/, distributed deps, SKILL.md, native tool binary"
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

## Step 8a: Reconcile Runtimes and Regenerate Command Aliases

This step has two phases:

1. **Reconcile detection with `config.md`** — detect which runtimes are on PATH and compare against `available-adapters`. If a new runtime appeared since last sync, prompt the user before changing config.
2. **Regenerate per-command aliases** — for every runtime in the detected set, write the corresponding alias files.

### Phase 1: Detect runtimes and reconcile config.md

First, detect runtimes and read the current `adapter` / `available-adapters` state. The detection mirrors `detect_runtimes()` in `xavier/install.sh` so the two paths agree on what counts as "installed."

```bash
# Detect runtimes via command -v
DETECTED_RUNTIMES=""
command -v claude >/dev/null 2>&1 && DETECTED_RUNTIMES="${DETECTED_RUNTIMES} claude-code"
command -v cursor >/dev/null 2>&1 && DETECTED_RUNTIMES="${DETECTED_RUNTIMES} cursor"
command -v codex >/dev/null 2>&1 && DETECTED_RUNTIMES="${DETECTED_RUNTIMES} codex"
DETECTED_RUNTIMES="${DETECTED_RUNTIMES# }"

# Read current adapter from config.md. Capture to end-of-line via sed —
# the previous `grep -o '...: *[^ ]*'` form truncated at the first space,
# which silently mangled the default `(not yet detected)` placeholder
# into the literal `(not`. Then normalize placeholder values to empty so
# the reconcile branch treats a fresh vault as "nothing wired yet"
# instead of as "primary is `(not`".
CURRENT_ADAPTER="$(sed -n 's/^- \*\*adapter\*\*: *\(.*\)/\1/p' "$XAVIER_HOME/config.md" 2>/dev/null | head -n 1)"
case "$CURRENT_ADAPTER" in
  ""|"(not yet detected)"|"(not yet configured)") CURRENT_ADAPTER="" ;;
esac

CURRENT_AVAILABLE_LINE="$(grep -o '\*\*available-adapters\*\*: *\[[^]]*\]' "$XAVIER_HOME/config.md" 2>/dev/null | head -n 1 | sed 's/.*\[\(.*\)\]/\1/' | tr -d ' ' | tr ',' ' ')"

# If available-adapters is missing, the single primary is the entire set
# so far. With a normalized-empty CURRENT_ADAPTER this leaves
# CURRENT_AVAILABLE empty, which is the correct fresh-vault state.
if [ -z "$CURRENT_AVAILABLE_LINE" ]; then
  CURRENT_AVAILABLE="$CURRENT_ADAPTER"
else
  CURRENT_AVAILABLE="$CURRENT_AVAILABLE_LINE"
fi

# Compute newly-detected runtimes (in DETECTED but not in CURRENT_AVAILABLE)
NEW_RUNTIMES=""
for rt in $DETECTED_RUNTIMES; do
  case " $CURRENT_AVAILABLE " in
    *" $rt "*) : ;;
    *) NEW_RUNTIMES="${NEW_RUNTIMES} $rt" ;;
  esac
done
NEW_RUNTIMES="${NEW_RUNTIMES# }"

echo "Detected: ${DETECTED_RUNTIMES:-<none>}"
echo "Current primary: ${CURRENT_ADAPTER:-<unset>}"
echo "Current available-adapters: ${CURRENT_AVAILABLE:-<unset>}"
echo "New since last sync: ${NEW_RUNTIMES:-<none>}"
```

**If `CURRENT_ADAPTER` is empty after normalization** (fresh vault that never went through `/xavier setup`), do **not** run the reconcile prompt. Self-update should not bootstrap initial configuration — tell the user to run `/xavier setup` first to wire the primary adapter, then proceed to Phase 2 so alias regeneration still happens for whatever's on PATH. The Phase 2 gating handles the empty-config case cleanly.

**If `NEW_RUNTIMES` is empty**, the detected set is already represented in `config.md` — skip the prompt and proceed silently to Phase 2.

**If `NEW_RUNTIMES` is non-empty**, the user has gained a runtime since last sync. **Prompt before touching `config.md`.** Use `AskUserQuestion`:

> Detected new runtime(s) since last sync: **{NEW_RUNTIMES}**. How should `config.md` be updated?
>
> - **Extend list, keep current primary** (recommended) — adds **{NEW_RUNTIMES}** to `available-adapters`; keeps `adapter: {CURRENT_ADAPTER}`.
> - **Extend list, switch primary** — adds **{NEW_RUNTIMES}** to `available-adapters` AND swaps the primary adapter.
> - **Skip** — leaves `config.md` untouched. Aliases still regenerate for detected runtimes.

This is a **hard interactive gate** — do not infer the answer, do not pick a default. Wait for the user.

If the user picks **Extend list, switch primary**, follow up with a second `AskUserQuestion` listing each entry in `NEW_RUNTIMES` as a selectable option (plus the current primary as a "keep" fallback). Wait for the user before continuing.

Apply the user's choice:

```bash
case "$RECONCILE_CHOICE" in
  extend_keep|extend_switch)
    # New available-adapters = union of current and detected, sorted, deduplicated
    NEW_AVAILABLE_LIST="$(printf '%s\n%s\n' "$CURRENT_AVAILABLE" "$DETECTED_RUNTIMES" | tr ' ' '\n' | grep -v '^$' | sort -u | tr '\n' ' ' | sed 's/ *$//')"
    ADAPTERS_LIST="$(echo "$NEW_AVAILABLE_LIST" | sed 's/ /, /g')"

    if grep -q "available-adapters" "$XAVIER_HOME/config.md" 2>/dev/null; then
      sed -i.bak "s/- \*\*available-adapters\*\*: .*/- **available-adapters**: [$ADAPTERS_LIST]/" "$XAVIER_HOME/config.md" 2>/dev/null && rm -f "$XAVIER_HOME/config.md.bak"
    else
      awk -v list="$ADAPTERS_LIST" '
        /^- \*\*adapter\*\*:/ { print; print "- **available-adapters**: [" list "]"; next }
        { print }
      ' "$XAVIER_HOME/config.md" > "$XAVIER_HOME/config.md.tmp" && mv "$XAVIER_HOME/config.md.tmp" "$XAVIER_HOME/config.md"
    fi

    # Switch primary only if user picked extend_switch and supplied NEW_PRIMARY
    if [ "$RECONCILE_CHOICE" = "extend_switch" ] && [ -n "${NEW_PRIMARY:-}" ]; then
      sed -i.bak "s/- \*\*adapter\*\*: .*/- **adapter**: $NEW_PRIMARY/" "$XAVIER_HOME/config.md" 2>/dev/null && rm -f "$XAVIER_HOME/config.md.bak"
    fi
    ;;
  skip)
    # No changes to config.md; alias regeneration still runs against DETECTED_RUNTIMES below
    :
    ;;
esac
```

### Phase 2: Regenerate per-command aliases

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

Otherwise, write an alias file for each of the following 20 commands. This human-readable list MUST stay in sync with the executable `COMMANDS` block below and with the `COMMANDS` table in `xavier/install.sh` — adding a skill in one place without the others causes upgrade-vs-fresh-install drift:

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
| ask | Answer focused questions about a repo using captured team knowledge |
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
ask|Answer focused questions about a repo using captured team knowledge
deps-update|Scan lockfile and regenerate stale dependency-skills
export|Export a vault note to your personal Obsidian vault
bug|File a bug report as a GitHub Issue in the Xavier upstream repo
feedback|Open a GitHub Discussion in the Xavier upstream repository
self-update|Update Xavier skills and references to the latest release
uninstall|Remove the Xavier vault and all symlinks
"
```

Regenerate aliases **only for runtimes the user actually has installed**, using each runtime's own alias layout. The Claude Code, Cursor, and Codex formats differ — they are NOT interchangeable — so this step must mirror `install_command_aliases()` in `xavier/install.sh` exactly:

- **Claude Code**: a single Markdown file at `~/.claude/commands/<prefix>-<cmd>.md` containing frontmatter + Skill-tool delegation instructions.
- **Cursor**: a directory at `~/.cursor/skills/<prefix>-<cmd>/` with `SKILL.md` inside, containing frontmatter + a "When user says /xavier <cmd>" trigger description.
- **Codex**: a directory at `~/.agents/skills/<prefix>-<cmd>/` with `SKILL.md` inside, containing frontmatter + a thin Xavier router delegation.

Gate each alias write on `$DETECTED_RUNTIMES` (already populated in Phase 1). A Claude-only user must NOT have Cursor/Codex stubs materialized in their skill roots — and vice versa. Within the runtimes that ARE detected, create root directories if they don't already exist (`mkdir -p`) so users who deleted an alias root still get clean regeneration.

```bash
echo "$COMMANDS" | while IFS='|' read -r cmd desc; do
  [ -z "$cmd" ] && continue

  case " $DETECTED_RUNTIMES " in
    *" claude-code "*)
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
      ;;
  esac

  case " $DETECTED_RUNTIMES " in
    *" cursor "*)
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
      ;;
  esac

  case " $DETECTED_RUNTIMES " in
    *" codex "*)
      mkdir -p "$HOME/.agents/skills/${ALIAS_PREFIX}-${cmd}"
      cat > "$HOME/.agents/skills/${ALIAS_PREFIX}-${cmd}/SKILL.md" << ALIASEOF
---
name: ${ALIAS_PREFIX}-${cmd}
description: ${desc}. Use when user says /xavier ${cmd}.
---

Route this request through the Xavier router.

1. Read the Xavier router from \${XAVIER_HOME:-~/.xavier}/SKILL.md (or ~/.xavier/SKILL.md if unset).
2. Follow the Router Lifecycle with subcommand: ${cmd}.
3. Pass through any remaining user arguments unchanged.
4. Stop when the routed ${cmd} command reaches an AskUserQuestion/confirm/wait gate or terminal handoff. Do not infer answers, choose filenames, invoke another Xavier command, or continue into follow-up work unless the user's newest message explicitly asks for it.
ALIASEOF
      ;;
  esac
done
```

After regeneration, write up to 60 alias files (20 commands × up to 3 detected runtimes) — the actual count depends on which runtimes the user has on PATH. Proceed to Step 9. If `install_command_aliases()` in `xavier/install.sh` ever changes its format, paths, or runtime set, this block must be updated to match.

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
