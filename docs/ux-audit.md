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

---

## Phase 2 — Shared References Audit

Phase 2 audits the shared reference files in `~/.xavier/references/` that multiple skills consume via the router's `requires` vocabulary. Cross-checks each reference against consuming skills for executor-clarity, consistency, and behavioral differentiation.

**Reference files audited:**
- `references/patterns/shark.md` — consumed by review, learn, loop, grill
- `references/adapters/claude-code/adapter.md` — consumed by grill, learn, review
- `references/adapters/ADAPTER-CONTRACT.md` — generic adapter contract
- `references/personas/*.md` — consumed by review
- `references/formats/zettelkasten.md` — consumed by learn, prd, tasks, review

---

### Finding P2-1: Shark protocol does not define `SHARK_TASK_HASH` — skills must set it themselves but nothing says who does
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `references/patterns/shark.md`, "Detect-and-Defer" section; all consuming skills' Step 1/3
- **Description**: The Shark protocol defines the detect-and-defer mechanism: check `SHARK_TASK_HASH` to decide whether to run as a shark or inline executor. All four consuming skills (review, learn, loop, grill) check this variable. However, nothing in the protocol or any skill specifies **who sets** `SHARK_TASK_HASH`. When a shark spawns a remora, the remora needs `SHARK_TASK_HASH` to be set in its environment so it knows it is nested. But the shark protocol's remora-spawning rules (lines 15-19) do not mention setting this variable on spawned agents, and the adapter contract's `spawn()` operation has no `env` parameter. This means the detect-and-defer mechanism has no defined activation path — the variable is checked everywhere but set nowhere.
- **Suggested fix**: Add explicit instructions to the Shark protocol's "Remora Spawning Rules" section: "When spawning a remora, set `SHARK_TASK_HASH` to a unique identifier for the current task (e.g., a hash of the task description). Pass it via the agent's environment or prepend `export SHARK_TASK_HASH=...` to the remora prompt." Also update the adapter contract's `spawn()` to accept an optional `env` map.

### Finding P2-2: Shark protocol says "spawn via the Agent tool" but adapter contract says "use adapter.spawn()"
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `references/patterns/shark.md` line 15; `references/adapters/claude-code/adapter.md` lines 11-25; `references/adapters/ADAPTER-CONTRACT.md`
- **Description**: The Shark protocol says "Spawn remoras via the Agent tool with `run_in_background: true`" — referencing the Claude Code Agent tool directly. The adapter contract defines a `spawn(task, options)` abstraction that maps to the Agent tool. The adapter exists precisely to decouple skills from a specific runtime. But the Shark protocol bypasses the adapter entirely by naming the Agent tool directly. Skills are inconsistent: review (Step 4) uses `Agent()` calls directly; grill (Step 3) also uses `Agent()` directly; learn (Step 4) uses `Agent()` with `subagent_type: "Explore"` which is not part of the adapter contract's `spawn()` interface. The adapter abstraction is effectively dead — every skill and the Shark protocol itself hardcode Claude Code's Agent tool.
- **Suggested fix**: Either (a) update the Shark protocol to reference `adapter.spawn()` instead of the Agent tool, and update skill examples to use the adapter vocabulary, or (b) acknowledge that Xavier is Claude Code-only and remove the adapter abstraction to reduce indirection. If keeping the adapter, add `subagent_type` to the adapter contract's `spawn()` options since learn already uses it.

### Finding P2-3: Adapter contract defines `poll()` but no skill or protocol ever calls it
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `references/adapters/ADAPTER-CONTRACT.md` lines 26-33; `references/adapters/claude-code/adapter.md` lines 28-31
- **Description**: The adapter contract defines three operations: `spawn()`, `poll()`, and `collect()`. The Claude Code adapter's `poll()` implementation says "Claude Code automatically notifies when background agents complete. No explicit polling is needed." This effectively makes `poll()` a no-op for the only existing adapter. No skill or the Shark protocol ever references `poll()`. The `collect()` operation is also never explicitly called — skills use the Pilot Fish pattern (process results as notifications arrive) rather than batch-collecting. The adapter contract defines an interface that nothing actually uses as designed.
- **Suggested fix**: Either remove `poll()` and `collect()` from the contract (they are artifacts of a generic design that does not match the notification-based reality), or document in the Shark protocol when `poll()` vs notification-based processing should be used.

### Finding P2-4: Adapter contract lives at `references/adapters/ADAPTER-CONTRACT.md` but router vocabulary points to `references/adapters/<name>/`
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/SKILL.md` requires vocabulary, `adapter` row; `references/adapters/ADAPTER-CONTRACT.md`; `references/adapters/claude-code/adapter.md`
- **Description**: The router's requires vocabulary says the `adapter` key loads "the adapter from `<vault>/references/adapters/` matching the `adapter` field in config." This resolves to the runtime-specific `adapter.md` (e.g., `adapters/claude-code/adapter.md`). The generic `ADAPTER-CONTRACT.md` sitting at the `adapters/` root is never loaded by any requires key — no skill declares a dependency on it, and the router has no vocabulary entry for it. This means the contract that defines what adapters must implement is invisible to the executor at runtime. Skills see the Claude Code adapter but not the contract it is supposed to implement, making it impossible for the executor to validate compliance.
- **Suggested fix**: Either (a) add a `adapter-contract` requires key that loads `ADAPTER-CONTRACT.md`, or (b) inline the contract's essential rules (the three operations and their signatures) into each adapter's `adapter.md` so the executor sees the full picture in one file.

### Finding P2-5: `learn` skill requires `[shark, adapter]` but inlines Zettelkasten format instead of requiring it
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `skills/learn/SKILL.md` frontmatter and Steps 4-7; `references/formats/zettelkasten.md`
- **Description**: The `learn` skill's `requires` list is `[config, shark, adapter, repo-conventions, team-conventions]`. It does not include `zettelkasten` (or any format reference). Yet the skill extensively uses Zettelkasten frontmatter — it hardcodes the full frontmatter schema inline in each remora prompt (Steps 4, 7). The `zettelkasten.md` reference defines the canonical schema including the `type` enum (`review, prd, tasks, knowledge, dependency`) and wikilink conventions. By inlining the schema rather than referencing it, `learn` risks schema drift: if `zettelkasten.md` is updated (e.g., a new required field), `learn`'s hardcoded prompts will not pick up the change. By contrast, `prd` and `tasks` explicitly reference `zettelkasten.md` in their prose ("see `~/.xavier/references/formats/zettelkasten.md`") but also do not declare it in `requires`.
- **Suggested fix**: Add a `zettelkasten` key to the requires vocabulary that loads `references/formats/zettelkasten.md`. Have `learn`, `prd`, `tasks`, and `review` declare it in their `requires` list. Remove inline schema duplication from `learn`'s remora prompts and instead reference the resolved context.

### Finding P2-6: Zettelkasten `type` enum does not include `tasks` despite skills using it
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `references/formats/zettelkasten.md` line 27; `skills/tasks/SKILL.md` Step 7
- **Description**: The Zettelkasten reference defines the `type` field as "one of: `review`, `prd`, `tasks`, `knowledge`, `dependency`" — this list includes `tasks`. However, looking at the type-specific fields section (lines 35-45), only `Reviews`, `PRDs`, and `Dependencies` have type-specific field definitions. There are no type-specific fields for `tasks`. The `tasks` skill writes frontmatter with `type: tasks` and includes a `source` field (wikilink to the originating PRD), but this `source` field is documented under "PRDs" in the Zettelkasten reference (line 42: `source: wikilink to the originating PRD (for task files)`). This is confusing — the `source` field is described under the PRDs heading but is actually used by tasks files.
- **Suggested fix**: Add a "### Tasks" subsection under "Type-Specific Fields" that documents the `source` field and any other task-specific fields. Move the `source` field description from the PRDs section to the Tasks section (or duplicate it if PRDs also use `source`).

### Finding P2-7: Personas have strong behavioral differentiation but inconsistent severity scales
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `references/personas/correctness.md`, `references/personas/security.md`, `references/personas/performance.md`
- **Description**: The three personas have excellent behavioral differentiation — each focuses on a distinct domain, explicitly instructs "do NOT comment on" other domains, and uses domain-specific output templates. However, their severity scales are inconsistent. Correctness uses: `critical`, `major`, `minor`. Performance uses: `critical`, `major`, `minor`. Security uses: `critical`, `high`, `medium`, `low`. The review skill's Pilot Fish (Step 5) must "normalize across persona severity scales" and lists a merged scale: `critical > high > major > medium > minor > low`. But the personas themselves do not document how their scales map to this merged scale. An executor must infer that correctness `major` equals the merged `major`, and security `high` sits between `critical` and `major`. This mapping is implicit and fragile.
- **Suggested fix**: Standardize all personas on the same severity scale (the merged one from the review skill: `critical`, `high`, `major`, `medium`, `minor`, `low`), or add an explicit mapping table to each persona (e.g., "This persona's `critical` maps to the review's `critical`"). The review skill already defines the canonical scale — push it upstream into the personas.

### Finding P2-8: Personas define an output format the review skill's Pilot Fish does not parse
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `references/personas/*.md` "Output Format" sections; `skills/review/SKILL.md` Step 5
- **Description**: Each persona defines a specific output format (e.g., correctness uses `### [severity] Short description` with `**File**:`, `**Scenario**:`, `**Suggestion**:`). Security uses a different template (`**Attack vector**:`, `**CWE**:`). Performance uses yet another (`**Impact**:`). The review skill's Pilot Fish (Step 5) must categorize, deduplicate, and rank findings from all three personas, but it receives free-form agent output — there is no structured parsing contract. The Pilot Fish instructions say to "categorize all findings by type" and "deduplicate" but do not specify how to extract findings from the heterogeneous output formats. An executor must pattern-match against three different templates, which is error-prone and may lead to missed findings or incorrect deduplication.
- **Suggested fix**: Either (a) standardize all personas on a single output format with consistent field names (e.g., always use `**File**:`, `**Category**:`, `**Severity**:`, `**Description**:`, `**Suggestion**:`), or (b) add explicit parsing instructions to the Pilot Fish step that describe how to extract findings from each persona's format.

### Finding P2-9: `review` skill declares `requires: [shark, personas, adapter, ...]` but `zettelkasten` is missing — review note uses the schema
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `skills/review/SKILL.md` frontmatter and Step 7
- **Description**: The review skill writes a review note to the vault (Step 7) using Zettelkasten frontmatter (`repo`, `type`, `verdict`, `finding-categories`, `recurring`, `tags`, `related`, `created`, `updated`). This frontmatter includes fields from the Zettelkasten schema plus review-specific extensions. However, the skill does not declare `zettelkasten` (or any format reference) in its `requires` list, and there is no such requires key in the vocabulary. The executor must know the Zettelkasten schema from the hardcoded example in Step 7 alone. If the canonical schema changes in `zettelkasten.md`, the review skill's hardcoded example will not update.
- **Suggested fix**: Same as P2-5 — add a `zettelkasten` requires key and have `review` declare it.

### Finding P2-10: `loop` skill requires `[shark]` but spawns agents without requiring `adapter`
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `skills/loop/SKILL.md` frontmatter and Step 4c
- **Description**: The loop skill's `requires` list is `[config, shark]`. It does not require `adapter`. Yet in Step 4c, it spawns remoras using `Agent()` calls with `run_in_background: true` — the same pattern that `review` and `grill` use, which both require `adapter`. The loop skill bypasses the adapter entirely and hardcodes Claude Code's Agent tool. If Xavier ever supports another runtime, loop would break silently because it never consults the adapter for how to spawn agents. Even within Claude Code, the executor has no adapter context to validate the spawning pattern.
- **Suggested fix**: Add `adapter` to loop's `requires` list and reference the resolved adapter context when spawning remoras, consistent with how review and grill do it.

### Finding P2-11: Shark state file 100-line limit is unenforceable and undefined in failure mode
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `references/patterns/shark.md` lines 43-50
- **Description**: The Shark protocol states "The state file must stay under 100 lines to avoid context bloat." This is a soft instruction to an LLM executor with no enforcement mechanism. The protocol does not say what to do if the state file exceeds 100 lines — should the executor truncate it, summarize old entries, rotate sections, or error? The loop skill (the primary consumer of state files) writes state to `~/.xavier/loop-state/` and updates it every iteration (Step 4g) with progress logs and learnings. Over 10+ iterations, this will easily exceed 100 lines. Without a defined pruning strategy, the state file will silently grow and eventually degrade context quality.
- **Suggested fix**: Define a concrete pruning strategy in the Shark protocol: "When the state file exceeds 100 lines, summarize completed phases into a single-line entry and keep only the 3 most recent learnings. Discard pass/fail history older than the last 5 iterations."

### Finding P2-12: `grill` skill requires `[shark, adapter]` but uses Shark only for detect-and-defer, not the full loop
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `skills/grill/SKILL.md` Steps 1-4; `references/patterns/shark.md`
- **Description**: The grill skill requires the Shark protocol and uses it for detect-and-defer (Step 1) and research remora spawning (Step 3). However, grill does not implement the full Shark evaluation loop (backpressure commands, state tracking, iteration limits, escalation). The interview phase (Step 4) is interactive and human-driven, not automated. This means grill uses a subset of the Shark protocol — specifically the concurrency and detect-and-defer parts — but not the evaluation loop. The Shark protocol presents itself as a single unified pattern. An executor reading the full Shark reference for grill's context will load rules about backpressure, state files, and iteration limits that do not apply, potentially causing confusion about what grill is supposed to do.
- **Suggested fix**: Either (a) split the Shark protocol into composable sub-protocols (e.g., `shark-concurrency.md` for spawning/collecting, `shark-detect-defer.md` for nesting, `shark-loop.md` for the full evaluation loop), or (b) add a note at the top of the Shark reference: "Skills may use a subset of this protocol. Check the skill's instructions for which parts apply."

---

## Phase 2 Summary

| ID | Title | Severity | Tag |
|----|-------|----------|-----|
| P2-1 | `SHARK_TASK_HASH` checked everywhere, set nowhere | `silent-failure` | `executor-facing` |
| P2-2 | Shark protocol hardcodes Agent tool, bypassing adapter | `confusion` | `executor-facing` |
| P2-3 | Adapter `poll()` and `collect()` are dead code | `friction` | `executor-facing` |
| P2-4 | `ADAPTER-CONTRACT.md` is never loaded by any requires key | `silent-failure` | `executor-facing` |
| P2-5 | `learn` inlines Zettelkasten schema instead of requiring it | `confusion` | `executor-facing` |
| P2-6 | Zettelkasten `source` field documented under wrong type heading | `silent-failure` | `executor-facing` |
| P2-7 | Personas use inconsistent severity scales | `confusion` | `executor-facing` |
| P2-8 | Persona output formats are heterogeneous with no parsing contract | `friction` | `executor-facing` |
| P2-9 | `review` uses Zettelkasten schema without declaring it | `friction` | `executor-facing` |
| P2-10 | `loop` spawns agents without requiring `adapter` | `silent-failure` | `executor-facing` |
| P2-11 | Shark state file 100-line limit has no pruning strategy | `friction` | `executor-facing` |
| P2-12 | `grill` loads full Shark protocol but only uses a subset | `friction` | `executor-facing` |

**Severity breakdown**: 4 silent-failure, 3 confusion, 5 friction
**Tag breakdown**: 0 user-facing, 12 executor-facing

**Cumulative totals (Phase 1 + Phase 2)**: 22 findings — 8 silent-failure, 6 confusion, 8 friction

---

## Phase 3 — Skills Tier 1 (Core Loop)

Phase 3 traces the four core-loop skills (`setup`, `review`, `learn`, `loop`) through happy paths and key failure paths, documenting UX papercuts found via simulated walkthrough.

---

### 3a — setup

**Happy path**: User runs `/xavier setup` on a fresh machine. `requires: []` means the vault gate is skipped (correct). Step 1 checks `~/.xavier/` -- does not exist, proceeds to Step 2a (scaffold quiz), Step 2b (interview), Step 2c (detect global skills), Step 3 (scaffold vault, write config, personas, adapter, symlinks, git init), Step 4 (confirmation). This path is well-structured with clear sequencing.

**Failure path — existing vault (re-run guard)**: Step 1 detects existing vault with `config.md`, offers "Update preferences" or "Skip setup". This is a good guard. However, see findings below.

**Failure path — missing git**: Step 3f runs `git init && git add -A && git commit`. If `git` is not installed, this fails. No pre-flight check for git existence.

**Failure path — file permission errors**: Step 3 creates `~/.xavier/` and subdirectories. If the home directory has restrictive permissions (e.g., read-only filesystem), the skill will fail mid-scaffold with no recovery guidance.

### Finding P3-1: Setup scaffold tree in SKILL.md does not match the actual directory structure created
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/setup/SKILL.md`, Step 3 (directory tree diagram) vs Steps 3a-3d
- **Description**: The scaffold tree diagram in Step 3 shows `personas/`, `adapters/`, `skills/`, `knowledge/`, `prd/`, `tasks/`, `review-state/`, and `loop-state/` as direct children of `~/.xavier/`. But Step 3c copies personas to `~/.xavier/personas/` AND the references exist at `~/.xavier/references/personas/`. The tree does not show `references/` at all, despite it being a critical directory that houses patterns, personas templates, adapters, and formats. The router's requires vocabulary resolves personas from `<vault>/references/personas/` not `<vault>/personas/`. This means the tree diagram is incomplete — an executor following only the tree would miss creating `references/` and its subdirectories.
- **Suggested fix**: Update the scaffold tree to include `references/` with its subdirectories (`patterns/`, `personas/`, `adapters/`, `formats/`). Clarify the relationship between `references/personas/` (templates) and `personas/` (active copies with emphasis tuning).

### Finding P3-2: Setup does not pre-check for `git` before scaffolding
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/setup/SKILL.md`, Step 3f (Initialize Git)
- **Description**: Step 3f runs `git init && git add -A && git commit`. If git is not installed, this fails after the vault has already been partially scaffolded (directories and files created in Steps 3a-3d). The user is left with an incomplete vault — files exist but no git history. Re-running setup will hit the "already exists" guard (Step 1) and offer only "Update preferences" or "Skip", neither of which re-runs git init.
- **Suggested fix**: Add a pre-flight check at the start of Step 2a (or before Step 3): run `git --version` and fail immediately with a clear message if git is not found. Alternatively, make the "already exists" guard (Step 1) smarter: if the vault directory exists but is not a git repo, offer to initialize git.

### Finding P3-3: Persona emphasis tuning has no defined effect on reviewer behavior
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/setup/SKILL.md`, Step 3c; `references/personas/*.md`
- **Description**: Step 3c sets an `emphasis` field in each persona's frontmatter based on the user's review-priorities answer (e.g., correctness=high, security=medium for "correctness-first"). The persona files have an `emphasis` field in their frontmatter. However, the review skill (Step 4) loads all three personas and spawns all three reviewers regardless of emphasis level. Nothing in the reviewer prompt or the Pilot Fish aggregation uses the `emphasis` value to adjust behavior. A persona with `emphasis: medium` gets the exact same prompt, the same weight in deduplication, and the same influence on the final verdict as one with `emphasis: high`. The tuning is recorded but never consumed.
- **Suggested fix**: Either (a) use the emphasis field in the review skill — e.g., only spawn "high" personas as full reviewers and "medium" personas as optional/lightweight checks, or adjust the Pilot Fish's severity ranking to weight findings from high-emphasis personas more heavily, or (b) remove the emphasis tuning from setup if it has no behavioral effect.

### Finding P3-4: Smoke test uses `run_in_background: false` contrary to how adapters are actually used
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/setup/SKILL.md`, Step 3d (Detect Runtime & Wire Adapter)
- **Description**: The adapter smoke test spawns an agent with `run_in_background: false`. But every actual skill usage spawns agents with `run_in_background: true` (review, learn, loop all use background agents). The smoke test validates foreground agent execution but does not test the background spawning path that all skills depend on. A setup could pass the smoke test but then fail at review time because background agent spawning has a different failure mode.
- **Suggested fix**: Run the smoke test with `run_in_background: true` to match real-world usage, then wait for the result notification.

### Finding P3-5: Setup copies personas to two locations but the router only reads one
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/setup/SKILL.md`, Step 3c; `xavier/SKILL.md` requires vocabulary, `personas` row
- **Description**: Step 3c copies reference personas from `~/.xavier/references/personas/` to `~/.xavier/personas/`. The emphasis field is set on the copies in `~/.xavier/personas/`. But the router's `personas` requires key reads from `<vault>/references/personas/` — the original templates, not the emphasis-tuned copies. This means the review skill always receives the untuned reference personas. The copies in `~/.xavier/personas/` with the user's emphasis preferences are never read by any skill.
- **Suggested fix**: Either (a) change the router's `personas` requires key to read from `<vault>/personas/` (the tuned copies), or (b) have setup modify the files in `<vault>/references/personas/` directly instead of creating separate copies.

---

### 3b — review

**Happy path**: User runs `/xavier review` with uncommitted changes. Router resolves requires: shark, personas, adapter, recurring-patterns, repo-conventions, team-conventions. Skill checks SHARK_TASK_HASH (unset), runs pre-flight (reads adapter, detects diff), loads vault context, spawns 3 reviewer remoras concurrently, Pilot Fish aggregates as each completes, delivers verdict, writes review note, writes shark state. This path is well-designed with good incremental progress reporting.

**Failure path — no PR context / no diff**: Step 2 checks both `git diff` and `git diff --staged`. If both are empty, the skill tells the user and stops. This is handled correctly.

**Failure path — empty personas dir**: The router provides empty result for `personas`. The review skill loads "all three personas from the resolved `personas` context" (Step 4) but if the context is empty, there are no personas to load and no reviewers to spawn. The skill has no fallback for this scenario.

**Failure path — empty recurring-patterns**: Handled well — Step 3 says "If fewer than 2 reviews exist... omit it from reviewer prompts." Step 4 says "only included if patterns were extracted." This is clean.

**Failure path — adapter mismatch**: Step 2 says "If no adapter is wired, warn and fall back to inline execution." This is a good degradation path.

### Finding P3-6: Review skill has no fallback when personas resolve to empty
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/review/SKILL.md`, Step 4 (Spawn Reviewer Remoras)
- **Description**: Step 4 says "Load all three personas from the resolved `personas` context." If the personas directory is empty (deleted, corrupted, or vault scaffolded incorrectly — see P3-5), the resolved context is an empty list. The skill provides no instruction for what to do — it would attempt to spawn 3 reviewers with empty persona definitions, producing meaningless reviews. Unlike the adapter (which has a fallback in Step 2) and recurring-patterns (which has a graceful omission path), personas have no empty-state handling despite being the most critical input.
- **Suggested fix**: Add a guard at the start of Step 4: "If the resolved personas context is empty, warn the user: 'No reviewer personas found. Run /xavier setup to reinstall.' Stop execution."

### Finding P3-7: Review Step 6 lists severity as `critical > major > minor` but Step 5 uses a 6-level scale
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/review/SKILL.md`, Step 5 vs Step 6
- **Description**: Step 5 (Pilot Fish) defines the merged severity scale as "critical > high > major > medium > minor > low" (6 levels). But Step 6 (Deliver Verdict) says "List findings grouped by severity (critical > major > minor)" — only 3 levels. The executor must reconcile these: should the final output use the 6-level merged scale from Step 5 or collapse to the 3-level scale in Step 6? This inconsistency means findings at the `high`, `medium`, and `low` severity levels have no defined position in the Step 6 output grouping.
- **Suggested fix**: Use the same severity scale in both steps. If the full 6-level scale is canonical, update Step 6 to say "grouped by severity (critical > high > major > medium > minor > low)." If the 3-level grouping is intentional for output simplicity, document the mapping (e.g., critical+high -> critical group, major+medium -> major group, minor+low -> minor group).

### Finding P3-8: Review writes state to `review-state/` but no skill ever reads it
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/review/SKILL.md`, Step 8 (Write Shark State)
- **Description**: Step 8 writes a state file to `~/.xavier/review-state/{repo-name}.md` containing the last review date, verdict, and finding counts. However, no skill ever reads from `review-state/`. The review skill's own recurring-patterns come from `knowledge/reviews/` (Step 3), not `review-state/`. The loop skill has its own `loop-state/` directory. The setup skill creates the `review-state/` directory but nothing consumes its contents. This is dead state — written but never read, consuming vault space and git history without providing value.
- **Suggested fix**: Either (a) have the review skill's Step 2 or Step 3 read `review-state/` to provide continuity context (e.g., "Last review was 3 days ago, verdict was approve"), or (b) remove Step 8 and the `review-state/` directory entirely since `knowledge/reviews/` already stores the persistent review record.

### Finding P3-9: Repo-level persona overrides (`.xavier/personas/`) bypass the requires resolution
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/review/SKILL.md`, Step 3 point 3; `xavier/SKILL.md` requires vocabulary `personas` row
- **Description**: Step 3 says "Check if `.xavier/personas/` exists in the current repo. If so, those personas override the global ones." This override logic lives inside the skill, not in the router's requires resolution. The router's `personas` key always reads from `<vault>/references/personas/` (or `<vault>/personas/` if P3-5 is fixed). The skill then does a second read from the current repo's `.xavier/personas/` to override what the router provided. This means the requires system's resolution is incomplete — the router does not know about repo-level overrides. Skills that might consume personas in the future would need to duplicate this override logic.
- **Suggested fix**: Move the override logic into the router's `personas` requires resolution: "Read from `<vault>/references/personas/`. If `.xavier/personas/` exists in the current working directory's repo root, prefer those files instead." This way all skills get the correct personas without duplicating logic.

### Finding P3-10: Review note filename uses HEAD hash but the diff may not match HEAD
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/review/SKILL.md`, Step 7 (Write Review Note)
- **Description**: The review note filename uses `git rev-parse HEAD` for the short hash. But the review operates on `git diff` (unstaged changes) and `git diff --staged` (staged changes) — neither of which are committed. HEAD points to the last commit, not the current working state. If the user has uncommitted changes, the filename hash refers to a commit that does not contain the reviewed code. This makes the filename misleading for historical reference — looking up the hash in git history will show a different state than what was actually reviewed.
- **Suggested fix**: Either (a) use a hash of the diff content itself (e.g., `echo "<diff>" | git hash-object --stdin | cut -c1-7`) to uniquely identify the reviewed changes, or (b) document this limitation and accept HEAD as "approximate context", or (c) use a timestamp-based identifier instead of a git hash.

---

### 3c — learn

**Happy path**: User runs `/xavier learn` in a repo. Router resolves requires: config, shark, adapter, repo-conventions, team-conventions. Skill resolves repo name, checks for monorepo, checks for existing notes (re-run guard), resolves team ownership, checks SHARK_TASK_HASH, spawns 3 research remoras (architecture, decisions, dependencies), Pilot Fish writes notes progressively, delegates to add-dep for key dependencies, handles monorepo workspace dependencies. Well-structured with good progressive feedback.

**Failure path — no repo context**: Step 1 runs `git rev-parse --show-toplevel`. If not in a git repo, this fails. No error handling specified.

**Failure path — empty team-conventions / repo-conventions**: These resolve to empty via the router. Step 2 resolves team from config. If config has no teams, the user can type a new one. Handled correctly.

**Failure path — monorepo edge cases**: Step 7 handles monorepo mode, spawning per-workspace agents. If a workspace has no `package.json`, the behavior is undefined.

### Finding P3-11: Learn has no error handling when not in a git repository
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/learn/SKILL.md`, Step 1 (Re-run Guard)
- **Description**: Step 1 runs `basename $(git rev-parse --show-toplevel)` to resolve the repo name. If the user runs `/xavier learn` outside of a git repository, `git rev-parse --show-toplevel` exits with an error ("fatal: not a git repository"). The skill provides no handling for this case — no pre-check, no friendly error message. The executor will encounter a bash error and must improvise a response.
- **Suggested fix**: Add a pre-check: "Run `git rev-parse --show-toplevel`. If it fails, tell the user: 'Not in a git repository. Run /xavier learn from inside a git repo.' Stop execution."

### Finding P3-12: Learn Step 3 (Detect-and-Defer) comes after Step 2 (Team Resolution) which has user interaction
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/learn/SKILL.md`, Steps 2 and 3 ordering
- **Description**: Step 2 (Team Resolution) uses `AskUserQuestion` to ask the user which team owns the repo. Step 3 then checks `SHARK_TASK_HASH` — if set, the skill was spawned as a nested agent inside an outer Shark loop and should skip the full flow. But if learn is running as a nested agent, Step 2's interactive prompt will fire first, blocking on user input that will never come (the "user" is another agent). The detect-and-defer check should be the very first step (as it is in the review skill's Step 1), before any interactive prompts.
- **Suggested fix**: Move Step 3 (Detect-and-Defer) before Step 2 (Team Resolution). The ordering should be: Step 1 (re-run guard, repo detection), Step 2 (detect-and-defer), Step 3 (team resolution), Step 4 (spawn remoras).

### Finding P3-13: Learn only checks `package.json` workspaces — misses non-JS monorepos
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/learn/SKILL.md`, Step 1 point 2 and Step 7
- **Description**: Monorepo detection checks "the root `package.json` for a `workspaces` field." This only works for JavaScript/TypeScript monorepos using npm/yarn/pnpm workspaces. Monorepos using Go modules, Rust Cargo workspaces (`Cargo.toml` with `[workspace]`), Python monorepos (pants, bazel), Java multi-module projects (Maven/Gradle), or even JS monorepos using Nx/Turborepo without a `workspaces` field in `package.json` will be classified as `monorepo: false`. The entire Step 7 (workspace dependencies) will be skipped.
- **Suggested fix**: Expand monorepo detection to check multiple signals: `package.json` workspaces, `pnpm-workspace.yaml`, `Cargo.toml` `[workspace]`, `go.work`, Gradle `settings.gradle` with `include`, `BUILD` files. Alternatively, document the limitation: "Note: monorepo detection currently supports npm/yarn/pnpm workspaces only."

### Finding P3-14: Learn delegates to `/xavier add-dep` in Step 6 but does not specify invocation mechanism
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/learn/SKILL.md`, Step 6 (Add-dep Delegation)
- **Description**: Step 6 says "For each selected package, delegate to `/xavier add-dep <package-name>`. Do NOT duplicate the add-dep logic inline — invoke the skill directly." But the skill is running inside the router's "Execute inline" step (router Step 7). There is no defined mechanism for a skill to invoke another skill during execution. The router resolves and executes one skill per invocation. "Invoke the skill directly" could mean: (a) the executor should re-enter the router with a new `/xavier add-dep` command, (b) the executor should read and inline the add-dep SKILL.md, or (c) the executor should spawn a sub-agent that runs `/xavier add-dep`. None of these are specified. Option (a) would bypass the current skill's execution flow; option (b) contradicts "do NOT duplicate the logic inline"; option (c) requires adapter context that learn has but the sub-agent may not.
- **Suggested fix**: Define a skill-to-skill invocation mechanism in the router. For example: "To delegate to another skill, spawn a remora with the full `/xavier <command>` invocation as its prompt. The remora will trigger the router, which will resolve requires and execute the sub-skill independently."

### Finding P3-15: Learn remora prompts use `subagent_type: "Explore"` which is not in the adapter contract
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/learn/SKILL.md`, Step 4 (all three remora spawns); `references/adapters/ADAPTER-CONTRACT.md`
- **Description**: All three research remoras in Step 4 are spawned with `subagent_type: "Explore"`. This parameter does not exist in the adapter contract's `spawn()` interface and is not documented anywhere. It appears to be a Claude Code-specific parameter that gives the sub-agent broader file exploration permissions. This is another instance of skills hardcoding Claude Code specifics (see P2-2). An executor using a different adapter would not know what to do with `subagent_type`.
- **Suggested fix**: Either (a) add `subagent_type` to the adapter contract's `spawn()` options with defined values and semantics, or (b) document it as a Claude Code-specific extension in the adapter, or (c) remove it if the default agent type already has sufficient file access for exploration.

---

### 3d — loop

**Happy path**: User runs `/xavier loop`. Router resolves requires: config, shark. Skill gathers task source (file or freeform), runs pre-flight (backpressure commands, git state, task readability, stale state check), initializes state, runs the evaluation loop (read state, spawn remora, evaluate output, run backpressure, commit checkpoint, update state, decide next action). This is the most complex skill and the Shark protocol's primary consumer.

**Failure path — no task file**: Step 1 accepts either a task file from `~/.xavier/tasks/` or a freeform description. If no tasks exist and no freeform is given, the user is prompted. Handled reasonably.

**Failure path — backpressure command fails repeatedly**: Step 4h says "If 2 consecutive failures with no progress, stop and escalate to user." This is good. But see findings below.

**Failure path — infinite loop guard**: Max iterations defaults to 10, warns at >25. Good guard.

**Failure path — partial completion resume**: Step 2 point 4 checks for stale loop state and asks to resume or start fresh. Good recovery path.

### Finding P3-16: Loop does not require `adapter` but spawns agents — inconsistent with review and learn
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/loop/SKILL.md` frontmatter `requires: [config, shark]`; Step 4c
- **Description**: The loop skill's `requires` list is `[config, shark]` — it does not include `adapter`. Yet Step 4c spawns remoras using `Agent()` with `run_in_background: true`, the same pattern review and learn use, both of which require `adapter`. This was already flagged as P2-10. The consequence for the loop specifically: the executor has no adapter context to validate agent spawning, and if the adapter were ever changed (e.g., different runtime), loop would silently use the wrong spawning mechanism while review and learn would correctly consult the adapter.
- **Suggested fix**: Add `adapter` to loop's `requires` list.

### Finding P3-17: Loop pre-flight requires all backpressure commands to pass before starting but some tasks create the test infrastructure
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/loop/SKILL.md`, Step 2 point 1
- **Description**: Step 2 says "Run every [backpressure] command now. All must exit 0. Pre-existing failures waste iterations." This is a reasonable guard for tasks that modify existing code. But for tasks that create new functionality (e.g., "add a REST API with tests"), the backpressure commands (e.g., `npm test -- --grep "REST API"`) will fail before the first iteration because the code and tests do not exist yet. The pre-flight blocks the loop from ever starting on greenfield tasks.
- **Suggested fix**: Allow backpressure commands to have a `skip-preflight: true` flag, or change the pre-flight check to "run every command and record baseline results" rather than requiring exit 0. Alternatively, document that freeform tasks should use completion-criteria-style commands (e.g., `test -f src/api.ts`) rather than test-suite commands for the pre-flight to pass.

### Finding P3-18: Loop's "no progress for 2 consecutive iterations" stall detection is ambiguous
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/loop/SKILL.md`, Step 4h; `references/patterns/shark.md` lines 38-39
- **Description**: Step 4h says "No progress for 2 consecutive iterations: announce stall, ask user for guidance." The Shark protocol (line 39) says "If 2 consecutive failures with no progress, stop and escalate to user." But "no progress" is undefined. Does it mean (a) the same backpressure commands fail with the same error output, (b) the remora output is semantically similar to the previous iteration, (c) no files changed between iterations, or (d) all backpressure commands return the same pass/fail status? Without a concrete definition, the executor must use subjective judgment, which may vary between runs.
- **Suggested fix**: Define "no progress" concretely: "No progress means all backpressure commands return the same exit codes AND error output as the previous iteration, indicating the remora's changes did not move toward completion."

### Finding P3-19: Loop commits with `git add -u` but new files from remoras need explicit `git add <new-files>`
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/loop/SKILL.md`, Step 4f (Commit Checkpoint)
- **Description**: Step 4f says `git add -u && git add <new-files>`. The `git add -u` only stages modifications and deletions of tracked files. For new files, the executor must run `git add <new-files>` — but how does the shark know which files the remora created? The remora "reports what it did" (per Shark protocol), but the report is free-form text. The shark must parse the remora's output to extract file paths, or run `git status --porcelain` to find untracked files. Neither approach is specified. Additionally, the instruction says "Never stage secrets or build artifacts" but provides no mechanism for the executor to distinguish new source files from build artifacts without a `.gitignore`.
- **Suggested fix**: Specify the mechanism: "After the remora completes, run `git status --porcelain` to identify new untracked files. Stage source files that are not in `.gitignore`. If unsure whether a file should be staged, skip it and log a warning in the state file."

### Finding P3-20: Loop state files are "ephemeral" but never cleaned up
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/loop/SKILL.md`, Step 3 and Step 4h
- **Description**: Step 3 says loop state files have "no Zettelkasten frontmatter (they are ephemeral tracking, not knowledge)." Step 4h says "All phases complete: announce success, clean up state." But "clean up state" is not defined — does it mean delete the state file, archive it, or mark it as complete? If the state file is deleted, the vault's git history preserves it but the user loses quick access to loop results. If it is kept, `~/.xavier/loop-state/` accumulates stale state files over time. Step 2 point 4 checks for "existing state for this task" and offers resume/fresh-start, suggesting state files persist. But the "All phases complete" path says "clean up state" suggesting deletion. These are contradictory.
- **Suggested fix**: Define the cleanup behavior explicitly: "On successful completion, rename the state file to `<task-name>_completed.md` (or delete it). On stall/max-iterations, keep the state file for resume capability."

### Finding P3-21: Loop has no mechanism for the shark to detect which phase a freeform task is in
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/loop/SKILL.md`, Steps 1 and 4a
- **Description**: For task-file mode, phases are extracted from the file (Step 1 point 2) with clear structure. For freeform mode, Step 1 point 3 asks for "completion criteria, backpressure commands, and max iterations" but does not ask for phases. The loop (Step 4) references "current phase" throughout (Steps 4a, 4b, 4c, 4h: "mark phase complete, advance to next phase"). A freeform task has no phases — it has a single description and completion criteria. The executor must either treat the entire freeform task as a single phase (making "advance to next phase" meaningless) or somehow decompose the freeform description into phases (which is not instructed). Step 4c's remora prompt says "Phase {N}" which has no meaning for freeform tasks.
- **Suggested fix**: Add explicit handling for freeform mode: "For freeform tasks, treat the entire task as a single phase. The loop iterates on that single phase until backpressure passes or max iterations are reached. The remora prompt should use the task description directly rather than 'Phase {N}'."

---

## Phase 3 Summary

| ID | Title | Severity | Tag |
|----|-------|----------|-----|
| P3-1 | Scaffold tree diagram missing `references/` directory | `confusion` | `executor-facing` |
| P3-2 | No pre-check for `git` before scaffolding | `friction` | `user-facing` |
| P3-3 | Persona emphasis tuning has no downstream effect | `silent-failure` | `executor-facing` |
| P3-4 | Smoke test uses foreground mode, not background | `friction` | `executor-facing` |
| P3-5 | Personas copied to two locations, router reads wrong one | `silent-failure` | `executor-facing` |
| P3-6 | No fallback when personas resolve to empty | `silent-failure` | `executor-facing` |
| P3-7 | Severity scale inconsistency between Step 5 and Step 6 | `confusion` | `executor-facing` |
| P3-8 | `review-state/` written but never read | `friction` | `executor-facing` |
| P3-9 | Repo-level persona overrides bypass requires resolution | `confusion` | `executor-facing` |
| P3-10 | Review note filename hash does not match reviewed code | `silent-failure` | `executor-facing` |
| P3-11 | No error handling when not in a git repository | `silent-failure` | `user-facing` |
| P3-12 | Detect-and-defer runs after interactive team prompt | `friction` | `user-facing` |
| P3-13 | Monorepo detection only supports JS workspaces | `friction` | `executor-facing` |
| P3-14 | Skill-to-skill invocation mechanism undefined | `confusion` | `executor-facing` |
| P3-15 | `subagent_type: "Explore"` not in adapter contract | `confusion` | `executor-facing` |
| P3-16 | Loop missing `adapter` in requires (duplicate of P2-10) | `silent-failure` | `executor-facing` |
| P3-17 | Pre-flight blocks greenfield tasks | `friction` | `user-facing` |
| P3-18 | "No progress" stall detection is ambiguous | `confusion` | `executor-facing` |
| P3-19 | New file staging mechanism unspecified | `friction` | `executor-facing` |
| P3-20 | Loop state cleanup behavior contradictory | `friction` | `user-facing` |
| P3-21 | Freeform tasks have no phase structure but loop assumes phases | `confusion` | `executor-facing` |

**Phase 3 severity breakdown**: 6 silent-failure, 7 confusion, 8 friction
**Phase 3 tag breakdown**: 5 user-facing, 16 executor-facing

**Cumulative totals (Phase 1 + Phase 2 + Phase 3)**: 43 findings — 14 silent-failure, 13 confusion, 16 friction
