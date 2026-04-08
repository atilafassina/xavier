---
name: xavier
description: AI Agent Orchestrator & Knowledge System. Use when user says "/xavier setup", "/xavier review", "/xavier grill", "/xavier add-dep", "/xavier remove-dep", "/xavier deps-update", "/xavier uninstall", or any "/xavier" command.
---

# Xavier — AI Agent Orchestrator

Xavier is an AI agent orchestrator that manages code reviews with diverse personas, dependency-skill knowledge, and a personal knowledge vault.

## Command Routing

Parse the user's input to determine the subcommand. The argument after `/xavier` determines which skill to load and execute.

If no subcommand is provided, list available commands by scanning `<XAVIER_HOME>/skills/` for directories containing `SKILL.md`. Read each skill's YAML frontmatter and display a table with columns **Command** and **Description**.

### Router Lifecycle

Follow these steps exactly for every `/xavier <command>` invocation:

**0. Resolve XAVIER_HOME**: before doing anything else, determine the vault location:
- Run `echo "$XAVIER_HOME"` via Bash.
- If the variable is set and non-empty, use that value as the vault root.
- If unset or empty, default to `~/.xavier/`.
- Store the resolved path. **All `~/.xavier/` references below mean this resolved vault root.**

**1. Parse sub-command**: extract the first argument after `/xavier`. Any remaining arguments are passed through to the skill.

**2. Resolve skill file**: look for `<XAVIER_HOME>/skills/<command>/SKILL.md`.

**3. Unknown command gate**: if the skill file does not exist, show an error:
> Skill not found. Run `/xavier setup` to install skills.

**4. Read frontmatter**: parse the skill file's YAML frontmatter to get the `requires` list.

**5. Vault gate**: if `requires` is non-empty (not `[]`), check that `<XAVIER_HOME>/config.md` exists. If it does not exist, tell the user:
> Xavier vault not found. Run `/xavier setup` first.

Then stop — do not execute the skill.

**6. Resolve requires**:

**6a. Auto-load config**: Read and parse `<XAVIER_HOME>/config.md`. Make the config context available for all subsequent resolution steps. Validate that the following fields exist: `name`, `teams`, `git-strategy`, `adapter`. Warn (do not fail) on any missing fields.

**6b. Resolve adapter** (if `adapter` is in the requires list): Read the adapter name from the config's `adapter` field. Load the adapter from `<XAVIER_HOME>/references/adapters/<adapter-name>/adapter.md`. This is a sub-step of config resolution — the adapter key depends on the config being loaded first.

**6c. Resolve remaining keys**: For each other key in the `requires` list (excluding `config` and `adapter`, which are already resolved), load the corresponding context using the [Requires Vocabulary](#requires-vocabulary) below. Make all resolved context available for the skill's execution.

**7. Execute inline**: read the skill body (everything after the frontmatter) and follow its instructions directly in the current conversation, with all resolved context available.

**8. Vault commit**: after the skill completes successfully, dispatch a vault commit if the vault exists. Read the git strategy from `<XAVIER_HOME>/config.md`:
- **auto-commit** or **batch-commit**: `cd <XAVIER_HOME> && git add -A && git commit -m "<command>: <short context>"`
- **batch-commit + auto-push**: same as above, then `git push`
- **user-driven**: skip — the user commits manually

Skills never mention or execute vault commits — the router owns this exclusively.

---

## Requires Vocabulary

> **Path note:** All paths below are relative to the resolved `XAVIER_HOME` (see step 0 of the Router Lifecycle). When you see `<vault>/`, substitute the resolved vault root (e.g., `~/.xavier/` by default, or whatever `$XAVIER_HOME` resolved to).

The following 13 keys are the only valid values in a skill's `requires` list:

| Key | What to load |
|-----|-------------|
| `config` | Read `<vault>/config.md` — **auto-loaded** for any skill with non-empty requires |
| `personas` | Read all `.md` files in `<vault>/references/personas/` (or repo overrides from `.xavier/personas/` if present) |
| `shark` | Read `<vault>/references/patterns/shark.md` |
| `adapter` | Read the adapter from `<vault>/references/adapters/` matching the `adapter` field in config — resolved as a sub-step of config |
| `recurring-patterns` | Extract recurring patterns from the 10 most recent review notes in `<vault>/knowledge/reviews/` for the current repo |
| `team-conventions` | Read files in `<vault>/knowledge/teams/` matching the current repo or team |
| `repo-conventions` | Read files in `<vault>/knowledge/repos/` matching the current repo |
| `prd-index` | List all `.md` files in `<vault>/prd/` with titles and frontmatter |
| `tasks-index` | List all `.md` files in `<vault>/tasks/` with titles and frontmatter |
| `skills-index` | List all directories in `<vault>/skills/` |
| `deps-index` | List all directories in `<vault>/deps/` |
| `vault-memory` | Read `<vault>/MEMORY.md` |

If a `requires` key cannot be resolved (e.g., directory is empty or doesn't exist), provide an empty result for that key — do not fail. The skill decides how to handle missing context.

---

## Reference Files

Shared references used across multiple skills live in `<vault>/references/`:

- **`<vault>/references/patterns/shark.md`** — Shark orchestration protocol (delegates, never implements; backpressure is truth; remora spawning rules)
- **`<vault>/references/formats/zettelkasten.md`** — Base Zettelkasten frontmatter schema for vault notes
- **`<vault>/references/personas/`** — Default reviewer personas (correctness, security, performance)
- **`<vault>/references/adapters/`** — Runtime adapter contracts and implementations
