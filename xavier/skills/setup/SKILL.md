---
name: setup
requires: []
---

# Setup

Create and configure the Xavier vault at `~/.xavier/`.

## Step 1: Check for Existing Vault

Check if `~/.xavier/` already exists.

- **If it exists and has `config.md`**: the vault is already set up. Ask the user:
  - **Update preferences** — re-run the interview (Step 2b) and update `config.md` without touching the directory structure or git history. Existing vault content is preserved.
  - **Skip setup** — do nothing.
  - Do NOT delete or overwrite the existing vault.
- **If it does not exist**: proceed to Step 2a.

## Step 2a: Quiz the User (Scaffold)

Ask the user two questions before creating the vault:

1. **Repository name** — The name for the vault's git remote (default: `xavier-ai`)
2. **Visibility** — Whether the repo should be public or private (default: private)

## Step 2b: Interview the User (Personalization)

Run this interview whether this is a fresh setup or a preference update. Use AskUserQuestion for each.

1. **Name** — "What name should Xavier use for you?"
2. **Teams** — "What teams do you work with? (comma-separated, e.g. 'platform, mobile, infra')"
3. **Workflow preferences** — "Describe your typical workflow in a few words (e.g. 'trunk-based, small PRs, ship daily')"
4. **Review priorities** — "What kind of reviews matter most to you?" Options:
   - Correctness first (bugs, logic errors, edge cases)
   - Security first (auth, injection, data exposure)
   - Performance first (latency, memory, scaling)
   - Balanced (equal weight across all three)
5. **Git strategy** — "How should Xavier commit changes to the vault?" Options:
   - `auto-commit` — commit after every vault write
   - `batch-commit` (Recommended) — commit at the end of each command
   - `user-driven` — never auto-commit, user commits manually
   - `batch-commit + auto-push` — batch-commit and push to remote
6. **Export vault path** — "Where is your personal Obsidian vault? (optional — used by /xavier export to sync notes)" This question is **skippable** — if the user skips or leaves it blank, no `## Export` section is written and `/xavier export` will ask for the path later. If provided, store as `export-vault-path` under a `## Export` section in `config.md` (see Step 3a).

## Step 2c: Detect Existing Global Skills

Check if `~/.agents/skills/`, `~/.claude/commands/`, or `~/.cursor/skills/` directories exist. If found, list them and note coexistence — Xavier does not conflict with existing skill installations.

## Step 3: Scaffold the Vault

> Skip this step if the vault already exists (preference update flow).

Create the full directory tree:

```
~/.xavier/
├── config.md
├── MEMORY.md
├── personas/
│   ├── correctness.md
│   ├── security.md
│   └── performance.md
├── adapters/
├── skills/
├── deps/
├── knowledge/
│   ├── repos/
│   ├── teams/
│   └── reviews/
├── prd/
├── tasks/
└── loop-state/
```

Create each directory. Then write the files described in Steps 3a-3d.

### Step 3a: config.md

Write `~/.xavier/config.md` using the interview answers:

```markdown
---
version: 1
---

# Xavier Configuration

## User

- **name**: {answer from interview}
- **teams**: [{answer from interview}]

## Preferences

- **git-strategy**: {answer from interview}
- **workflow**: {answer from interview}
- **review-priorities**: {answer from interview — e.g. "correctness-first" or "balanced"}

## Runtime

- **adapter**: {detected adapter name, e.g. "claude-code" — see Step 3e}

## Export

- **export-vault-path**: {answer from interview, or omit this section entirely if skipped}
- **export-show-diff**: false
```

> **Note**: The `## Export` section is only written if the user provided an export vault path in question 6. If they skipped the question, omit the entire section.

### Step 3b: MEMORY.md

```markdown
# Xavier Memory Index

_No memories yet. Xavier will populate this as it learns about your codebase and preferences._
```

### Step 3c: Personas

Copy the three default personas from the references directory into the vault:

- `~/.xavier/references/personas/correctness.md` -> `~/.xavier/personas/correctness.md`
- `~/.xavier/references/personas/security.md` -> `~/.xavier/personas/security.md`
- `~/.xavier/references/personas/performance.md` -> `~/.xavier/personas/performance.md`

### Step 3d: Detect Runtime & Wire Adapter

Detect the active AI agent runtime and install the appropriate adapter. Xavier supports multiple runtimes — detect all that are available, wire adapters for each, and select a primary.

1. **Detection**: Check which tools are available in the current session:
   - If `Agent` tool AND `Bash` tool are available → runtime is **claude-code**
   - If `Task` tool AND `Shell` tool are available → runtime is **cursor**
   - If neither set of tools is detected → runtime is **unknown** (warn the user, skip adapter wiring)

   A session may match only one runtime. Use the first match as the primary adapter.

2. **Wire adapters**: For each detected runtime, copy the adapter files from `~/.xavier/references/adapters/<runtime>/` to `~/.xavier/adapters/<runtime>/`

3. **Update config**: Set the `adapter` field in `~/.xavier/config.md` to the primary runtime name (e.g., `claude-code` or `cursor`). If multiple runtimes were detected, also set `available-adapters` to the full list.

4. **Smoke test**: Spawn a trivial agent through the detected adapter to verify it works.

   For **claude-code**:
   ```
   spawn(
     task: "Reply with exactly: 'Xavier adapter smoke test passed'",
     options: { name: "xavier smoke test", background: false }
   )
   ```

   For **cursor**:
   ```
   spawn(
     task: "Reply with exactly: 'Xavier adapter smoke test passed'",
     options: { name: "xavier smoke test", background: false, subagent_type: "generalPurpose" }
   )
   ```

   If the agent returns the expected output, the adapter is working. Report success. If it fails, warn the user but don't block setup.

### Step 3e: Register Skill Symlinks

Create symlinks so Xavier is registered as a global skill across all supported runtimes. Derive the repo path from the skill's own base directory (go up from this skill file through `skills/setup/` to reach the `xavier/` directory, and up one more for the repo root).

1. **Symlink 1**: `~/.agents/skills/xavier/` → the `xavier/` directory in the repo
   - Create parent directory `~/.agents/skills/` if it doesn't exist
   - If the symlink already exists, warn the user and skip — do NOT overwrite
   - If it doesn't exist, create it: `ln -s <repo>/xavier ~/.agents/skills/xavier`

2. **Symlink 2** (Claude Code): `~/.claude/commands/xavier.md` → `xavier/SKILL.md` in the repo
   - Create parent directory `~/.claude/commands/` if it doesn't exist
   - If the symlink already exists, warn the user and skip — do NOT overwrite
   - If it doesn't exist, create it: `ln -s <repo>/xavier/SKILL.md ~/.claude/commands/xavier.md`

3. **Symlink 3** (Cursor): `~/.cursor/skills/xavier/SKILL.md` → `xavier/SKILL.md` in the repo
   - Create parent directory `~/.cursor/skills/xavier/` if it doesn't exist
   - If the symlink already exists, warn the user and skip — do NOT overwrite
   - If it doesn't exist, create it: `ln -s <repo>/xavier/SKILL.md ~/.cursor/skills/xavier/SKILL.md`

4. **Report**: Tell the user what was created and what was skipped.

### Step 3f: Initialize Git

Initialize the vault as a git repository (if not already one):

```bash
cd ~/.xavier && git init && git add -A && git commit -m "xavier: initial vault scaffold"
```

## Step 4: Confirm

Tell the user:
- The vault has been created (or preferences updated) at `~/.xavier/`
- Show the directory tree (for fresh setup) or the updated config fields (for preference update)
- List any detected global skills from Step 2c
- Remind them they can push to a remote with: `cd ~/.xavier && gh repo create <repo-name> --private --source=. --push`
