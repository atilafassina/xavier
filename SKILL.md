---
name: xavier
description: AI Agent Orchestrator & Knowledge System. Use when user says "/xavier setup", "/xavier review", "/xavier grill", "/xavier add-dep", "/xavier remove-dep", "/xavier deps-update", "/xavier uninstall", or any "/xavier" command.
---

# Xavier — AI Agent Orchestrator

Xavier is an AI agent orchestrator that manages code reviews with diverse personas, dependency-skill knowledge, and a personal knowledge vault.

## Command Routing

Parse the user's input to determine the subcommand. The argument after `/xavier` determines which skill to load and execute.

If no subcommand is provided, list available commands by scanning `~/.xavier/skills/` for directories containing `SKILL.md`. Read each skill's YAML frontmatter and display a table with columns **Command** and **Description**.

### Router Lifecycle

Follow these steps exactly for every `/xavier <command>` invocation:

**1. Parse sub-command**: extract the first argument after `/xavier`. Any remaining arguments are passed through to the skill.

**2. Resolve skill file**: look for `~/.xavier/skills/<command>/SKILL.md`.

**3. Unknown command gate**: if the skill file does not exist, show an error:
> Skill not found. Run `/xavier setup` to install skills.

**4. Read frontmatter**: parse the skill file's YAML frontmatter to get the `requires` list.

**5. Vault gate**: if `requires` is non-empty (not `[]`), check that `~/.xavier/config.md` exists. If it does not exist, tell the user:
> Xavier vault not found. Run `/xavier setup` first.

Then stop — do not execute the skill.

**6. Resolve requires**: for each key in the `requires` list, load the corresponding context using the [Requires Vocabulary](#requires-vocabulary) below. Make the resolved context available for the skill's execution.

**7. Execute inline**: read the skill body (everything after the frontmatter) and follow its instructions directly in the current conversation, with all resolved context available.

**8. Vault commit**: after the skill completes successfully, dispatch a vault commit if the vault exists. Read the git strategy from `~/.xavier/config.md`:
- **auto-commit** or **batch-commit**: `cd ~/.xavier && git add -A && git commit -m "<command>: <short context>"`
- **batch-commit + auto-push**: same as above, then `git push`
- **user-driven**: skip — the user commits manually

Skills never mention or execute vault commits — the router owns this exclusively.

---

## Requires Vocabulary

The following 12 keys are the only valid values in a skill's `requires` list:

| Key | What to load |
|-----|-------------|
| `config` | Read `~/.xavier/config.md` |
| `personas` | Read all `.md` files in `~/.xavier/references/personas/` (or repo overrides from `.xavier/personas/` if present) |
| `shark` | Read `~/.xavier/references/patterns/shark.md` |
| `adapter` | Read the adapter from `~/.xavier/references/adapters/` matching the `adapter` field in config |
| `recurring-patterns` | Extract recurring patterns from the 10 most recent review notes in `~/.xavier/knowledge/reviews/` for the current repo |
| `team-conventions` | Read files in `~/.xavier/knowledge/teams/` matching the current repo or team |
| `repo-conventions` | Read files in `~/.xavier/knowledge/repos/` matching the current repo |
| `prd-index` | List all `.md` files in `~/.xavier/prd/` with titles and frontmatter |
| `tasks-index` | List all `.md` files in `~/.xavier/tasks/` with titles and frontmatter |
| `skills-index` | List all directories in `~/.xavier/skills/` |
| `vault-memory` | Read `~/.xavier/MEMORY.md` |

If a `requires` key cannot be resolved (e.g., directory is empty or doesn't exist), provide an empty result for that key — do not fail. The skill decides how to handle missing context.

---

## Reference Files

Shared references used across multiple skills live in `~/.xavier/references/`:

- **`~/.xavier/references/patterns/shark.md`** — Shark orchestration protocol (delegates, never implements; backpressure is truth; remora spawning rules)
- **`~/.xavier/references/formats/zettelkasten.md`** — Base Zettelkasten frontmatter schema for vault notes
- **`~/.xavier/references/personas/`** — Default reviewer personas (correctness, security, performance)
- **`~/.xavier/references/adapters/`** — Runtime adapter contracts and implementations
