# UX Papercut Audit -- Phase 1: Router Audit

Phase 1 traces the Xavier router (`xavier/SKILL.md`) end-to-end across 5 surfaces, documenting happy paths, failure paths, and findings.

**Router file**: `/Users/atila.fassina/Developer/xavier/xavier/SKILL.md`

---

## Surface 1: Command Parsing and Unknown-Command Error

### Happy path

User runs `/xavier review`. The router:
1. Resolves `XAVIER_HOME` (step 0).
2. Extracts `review` as the subcommand (step 1).
3. Looks for `<XAVIER_HOME>/skills/review/SKILL.md` -- file exists.
4. Proceeds to frontmatter parsing.

No issues found on the happy path.

### Failure path: unknown command

User runs `/xavier foobar`. The router checks for `<XAVIER_HOME>/skills/foobar/SKILL.md`, which does not exist. Error shown:

> Skill not found. Run `/xavier setup` to install skills.

### Finding P1-1: Unknown-command error does not echo the command back
- **Severity**: `confusion`
- **Tag**: `user-facing`
- **Location**: `xavier/SKILL.md`, step 3 (Unknown command gate)
- **Description**: The error message says "Skill not found" but does not include which command the user typed. If the user made a typo (`/xavier reveiw` instead of `/xavier review`), the error gives no hint about the actual input or close matches. Additionally, suggesting `/xavier setup` is misleading -- setup creates a vault, it does not install missing skills. If skills are already installed the suggestion is a dead end.
- **Suggested fix**: Echo the command name in the error: `Unknown command "foobar".` Follow with `Run /xavier to see available commands.` Consider adding fuzzy-match suggestions (e.g., "Did you mean: review?").

---

## Surface 2: Vault Gate (`config.md` existence check)

### Happy path

User runs `/xavier review`. The `requires` list is non-empty (`[shark, personas, adapter, ...]`). The router checks `<XAVIER_HOME>/config.md` -- it exists. Proceeds to resolve requires.

No issues found on the happy path.

### Failure path: vault not initialized

User runs `/xavier review` before running setup. `<XAVIER_HOME>/config.md` does not exist. Error shown:

> Xavier vault not found. Run `/xavier setup` first.

This is clear and actionable. No issues found.

### Failure path: skill with empty requires

User runs `/xavier setup` (which has `requires: []`). The vault gate is skipped entirely because requires is empty. This is correct -- setup must work without an existing vault.

No issues found.

### Finding P1-2: Vault gate checks config.md but not the vault directory itself
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/SKILL.md`, step 5 (Vault gate)
- **Description**: The vault gate only checks whether `config.md` exists. If the vault directory (`~/.xavier/`) exists but `config.md` was deleted or corrupted, the gate fires and tells the user to run setup. However, if the vault directory does not exist at all, the gate fires with the same message -- which is correct. The subtler issue: if `config.md` exists but is empty or malformed (no YAML frontmatter, no git-strategy field), the gate passes but downstream steps (adapter resolution, vault commit) will silently get empty/wrong values.
- **Suggested fix**: After the gate passes, validate that `config.md` has the minimum required fields (at least `adapter` and `git-strategy`). If missing, emit a warning: `config.md is incomplete. Run /xavier setup to reconfigure.`

---

## Surface 3: `requires` Vocabulary Resolution -- Empty Key Behavior

### Happy path

User runs `/xavier review`. The `requires` list is `[shark, personas, adapter, recurring-patterns, repo-conventions, team-conventions]`. Each key resolves:
- `shark` reads `<vault>/references/patterns/shark.md` -- exists.
- `personas` reads `<vault>/references/personas/*.md` -- exists.
- `adapter` reads config to find `adapter: claude-code`, then reads `<vault>/references/adapters/claude-code/` -- exists.
- `recurring-patterns` reads recent reviews in `<vault>/knowledge/reviews/` for current repo -- may or may not have entries.
- `repo-conventions` reads `<vault>/knowledge/repos/` matching current repo -- may or may not have entries.
- `team-conventions` reads `<vault>/knowledge/teams/` matching current repo or team -- may or may not have entries.

Per line 73: "If a requires key cannot be resolved... provide an empty result for that key -- do not fail." This is a graceful design.

No issues on the happy path.

### Failure path: key resolves to empty

For keys like `recurring-patterns`, `team-conventions`, or `repo-conventions`, the directory may exist but contain no matching files (e.g., first review of a new repo). The router provides an empty result. The skill receives no context but must handle this itself.

### Finding P1-3: No signal to the user when critical context resolves to empty
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/SKILL.md`, step 6 (Resolve requires) and the requires vocabulary table
- **Description**: When a required key resolves to empty, the router silently provides an empty result. This is fine for optional context like `recurring-patterns`. But for context that fundamentally shapes behavior -- like `adapter` (needed to spawn background agents) or `personas` (needed for review) -- an empty resolution means the skill will run in a degraded mode without any indication to the user. The review skill does handle a missing adapter with a fallback warning, but this is skill-by-skill and inconsistent.
- **Suggested fix**: Distinguish between `requires` and `optional-requires` in skill frontmatter. For required context that resolves empty, emit a warning: `Warning: "personas" resolved to empty. Review quality may be reduced. Run /xavier learn to populate.`

### Finding P1-4: Requires vocabulary claims 12 keys but only lists 11
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/SKILL.md`, line 57 and the table at lines 59-71
- **Description**: The text states "The following 12 keys are the only valid values" but the table contains exactly 11 rows: `config`, `personas`, `shark`, `adapter`, `recurring-patterns`, `team-conventions`, `repo-conventions`, `prd-index`, `tasks-index`, `skills-index`, `vault-memory`. This off-by-one either means a key was removed without updating the count, or a key is missing from the table.
- **Suggested fix**: Audit the skills to see if any `requires` key is used but not listed. Update the count to match the actual table (11), or add the missing 12th key.

### Finding P1-5: `adapter` resolution requires `config` but does not declare dependency
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/SKILL.md`, requires vocabulary table, `adapter` row
- **Description**: The `adapter` key says "Read the adapter matching the `adapter` field in config." This means resolving `adapter` requires first reading `config.md` to find the adapter name. But `adapter` and `config` are independent keys in the requires list. If a skill declares `requires: [adapter]` without also listing `config`, the executor must implicitly read config to resolve adapter. This implicit dependency is undocumented and could lead to inconsistent behavior across executor implementations.
- **Suggested fix**: Document that `adapter` implicitly reads config, or enforce that any skill requiring `adapter` must also require `config`. Alternatively, make the adapter resolution self-contained by reading the config internally.

---

## Surface 4: No-Args Listing Behavior (Discoverability)

### Happy path

User runs `/xavier` with no arguments. Per line 14: "list available commands by scanning `<XAVIER_HOME>/skills/` for directories containing `SKILL.md`. Read each skill's YAML frontmatter and display a table with columns Command and Description."

### Finding P1-6: Most skills lack a `description` frontmatter field
- **Severity**: `confusion`
- **Tag**: `user-facing`
- **Location**: `xavier/SKILL.md`, line 14 (no-args behavior) and individual skill files
- **Description**: The no-args listing reads each skill's YAML frontmatter to display a Command/Description table. However, out of 15+ skills in the vault, only 2 have a `description` field (`babysit` and `grill`). All others (`review`, `learn`, `setup`, `export`, `prd`, `tasks`, `loop`, `add-dep`, `remove-dep`, `deps-update`, `self-update`, `uninstall`) lack this field. The resulting table will show empty descriptions for ~87% of commands, making the listing nearly useless for discoverability.
- **Suggested fix**: Add `description` to every skill's frontmatter. Consider making it a required field validated during `setup` or `self-update`. Alternatively, fall back to reading the first heading or paragraph of the skill body as a description.

### Finding P1-7: Scoped and dependency-type skills pollute the command listing
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/SKILL.md`, line 14 (no-args behavior)
- **Description**: The vault's skills directory contains scoped packages (`@databricks/sdk-experimental`) and dependency-type skills (`express`) alongside command skills. The no-args listing scans all directories in `<XAVIER_HOME>/skills/` for `SKILL.md` files. Skills with `type: dependency` in their frontmatter are knowledge entries, not user-invokable commands. Listing them alongside real commands (`review`, `setup`, etc.) confuses the user about what they can actually run. The `@databricks` scoped directory adds further ambiguity since its nested structure differs from the flat skill pattern.
- **Suggested fix**: Filter the listing to exclude skills with `type: dependency` in their frontmatter. Only show skills that are meant to be invoked as commands. Alternatively, add a `type: command` field to distinguish invokable skills.

---

## Surface 5: Vault Commit Dispatch (Git Strategy Handling)

### Happy path

Skill completes successfully. Router reads `<XAVIER_HOME>/config.md`, finds `git-strategy: batch-commit`. Router runs:

```
cd <XAVIER_HOME> && git add -A && git commit -m "review: <short context>"
```

No push (no auto-push configured). This works correctly.

### Failure path: git-strategy is `user-driven`

Router reads config, finds `git-strategy: user-driven`. Router skips the commit. Correct behavior.

### Failure path: nothing changed in the vault

Skill ran but made no file changes. Router runs `git add -A && git commit`, which fails because there is nothing to commit. Git returns a non-zero exit code.

### Finding P1-8: Vault commit fails noisily when there are no changes
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/SKILL.md`, step 8 (Vault commit)
- **Description**: The commit command `git add -A && git commit -m "..."` will fail with "nothing to commit, working tree clean" if the skill did not modify any vault files. This is a normal case -- not every skill invocation writes to the vault (e.g., a review with no new patterns to record, or an export that only reads). The user sees a git error after an otherwise successful skill execution, which is confusing.
- **Suggested fix**: Guard the commit: check if there are staged changes before committing. For example: `git add -A && git diff --cached --quiet || git commit -m "..."`.

### Finding P1-9: `batch-commit + auto-push` is parsed from prose, not a structured field
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/SKILL.md`, step 8 (Vault commit), lines 45-47
- **Description**: The config stores `git-strategy: batch-commit` as a single value. The router instructions describe three behaviors: `auto-commit`, `batch-commit`, and `batch-commit + auto-push`. The "auto-push" modifier is described as an addendum to `batch-commit` but there is no defined syntax for how this combination is expressed in `config.md`. An executor must guess whether the config should say `batch-commit-auto-push`, `batch-commit + auto-push`, or something else. The current config has `batch-commit` with no indication of how to opt into auto-push.
- **Suggested fix**: Define explicit, enumerable values for git-strategy: `auto-commit`, `batch-commit`, `batch-commit-push`, `user-driven`. Document these in the setup interview and validate them at the vault gate.

### Failure path: config.md has no git-strategy field

If `git-strategy` is missing from config (field was removed or config is minimal), the executor has no instruction for what to do. The router does not specify a default.

### Finding P1-10: No default git-strategy when field is missing from config
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/SKILL.md`, step 8 (Vault commit)
- **Description**: If `config.md` exists but does not contain a `git-strategy` field, the router gives no guidance on what the executor should do. Depending on the LLM's interpretation, it might skip committing (safe but surprising), commit anyway (unsafe assumption), or error. There is no explicit default or fallback.
- **Suggested fix**: Specify a default: "If `git-strategy` is not set in config, default to `user-driven` (safest)." Alternatively, validate during setup that git-strategy is always present.

---

## Summary

| ID | Title | Severity | Tag |
|----|-------|----------|-----|
| P1-1 | Unknown-command error does not echo the command back | `confusion` | `user-facing` |
| P1-2 | Vault gate checks config.md but not content validity | `silent-failure` | `executor-facing` |
| P1-3 | No signal when critical context resolves to empty | `silent-failure` | `executor-facing` |
| P1-4 | Requires vocabulary claims 12 keys but lists 11 | `friction` | `executor-facing` |
| P1-5 | `adapter` resolution implicitly depends on `config` | `silent-failure` | `executor-facing` |
| P1-6 | Most skills lack a `description` frontmatter field | `confusion` | `user-facing` |
| P1-7 | Scoped and dependency-type skills pollute command listing | `friction` | `user-facing` |
| P1-8 | Vault commit fails noisily when no changes exist | `friction` | `user-facing` |
| P1-9 | `batch-commit + auto-push` has no structured syntax | `confusion` | `executor-facing` |
| P1-10 | No default git-strategy when field is missing | `silent-failure` | `executor-facing` |

**Severity breakdown**: 4 silent-failure, 3 confusion, 3 friction
**Tag breakdown**: 4 user-facing, 6 executor-facing
