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

---

## Phase 4 — Skills Tier 2 (Planning)

Phase 4 traces the three planning skills (`prd`, `tasks`, `grill`) through happy paths and key failure paths, documenting UX papercuts found via simulated walkthrough.

---

### 4a — prd

**Happy path**: User runs `/xavier prd`. Router resolves `requires: [config, prd-index]`. Step 1 lists vault contents from `~/.xavier/prd/`, `~/.xavier/knowledge/repos/`, and `~/.xavier/knowledge/teams/` for context selection (multiSelect). Step 2 runs the interview: problem statement, codebase exploration, relentless questioning informed by vault context, module design, user quiz. Step 3 writes the PRD to `~/.xavier/prd/<filename>.md` with Zettelkasten frontmatter. The skill reminds the user about `/xavier export`. This path is well-structured with good progressive disclosure.

**Failure path -- no repo context**: Unlike `learn`, the prd skill never runs `git rev-parse` or assumes a git repo. The frontmatter includes `repo: {current repo name}` but the skill does not specify how to resolve the repo name. If run outside a git repo, the executor must improvise a value for the `repo` field.

**Failure path -- user abandons interview mid-way**: The interview in Step 2 has 5 sub-steps with user interaction at each. If the user abandons mid-interview (closes session, stops responding), there is no checkpoint or draft mechanism. All interview context is lost. The user must restart from scratch.

**Failure path -- vault path missing / no existing notes**: Step 1 point 4 says "If no notes exist in any of these directories, skip this step silently." This is correct and graceful. However, if `~/.xavier/prd/` does not exist as a directory (not just empty), the `prd-index` requires key will attempt to list files in a nonexistent directory. The router says to provide an empty result for unresolvable keys, so this should be handled, but it depends on executor interpretation.

**Failure path -- export fails or export-vault-path not configured**: The prd skill tells the user to run `/xavier export prd/<filename>`. If the user does and `export-vault-path` is not configured, the export skill handles this gracefully (asks the user). No issue here -- the indirection works.

### Finding P4-1: PRD skill does not specify how to resolve `repo` for frontmatter
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/prd/SKILL.md`, Step 3 (frontmatter template)
- **Description**: The frontmatter template includes `repo: {current repo name}` but the skill never specifies how to determine the current repo name. Other skills like `learn` explicitly run `basename $(git rev-parse --show-toplevel)`. The prd skill has no equivalent instruction. If run outside a git repo, the executor has no guidance. If run inside a git repo, the executor must guess to use `git rev-parse`. This is a small ambiguity, but given that `repo` is a required Zettelkasten field, it should have an explicit resolution mechanism.
- **Suggested fix**: Add a Step 0 or early instruction: "Resolve the current repo name via `basename $(git rev-parse --show-toplevel)`. If not in a git repo, ask the user for a project name to use in frontmatter."

### Finding P4-2: PRD interview has no checkpoint or draft mechanism -- abandoned sessions lose all progress
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/prd/SKILL.md`, Step 2 (Interview)
- **Description**: The interview flow has 5 sub-steps, each requiring user interaction: problem statement, codebase exploration, relentless questioning, module design, and user quiz. This can be a lengthy process. If the user's session ends mid-interview (network drop, terminal close, context window exhaustion), all gathered information is lost. There is no intermediate draft written to the vault, no session checkpoint, and no resume mechanism. By contrast, the loop skill has explicit state persistence and resume capability (Step 2 point 4). The prd skill's interview is equally long-running but has no equivalent.
- **Suggested fix**: After the problem statement and codebase exploration steps (Steps 2.1 and 2.2), write a draft to `~/.xavier/prd/_draft-<name>.md` with collected information so far. On re-invocation, check for existing drafts and offer to resume. Clean up drafts when the final PRD is written.

### Finding P4-3: PRD references Zettelkasten format via prose path but does not declare it in `requires`
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/prd/SKILL.md`, Step 3 and frontmatter `requires: [config, prd-index]`
- **Description**: Step 3 says "The PRD uses Zettelkasten frontmatter (see `~/.xavier/references/formats/zettelkasten.md`)" -- referencing the file by absolute path in prose. But the `requires` list is `[config, prd-index]` with no Zettelkasten entry. This means the Zettelkasten schema is not loaded into the executor's context via the requires system; the executor must follow the prose path hint and read the file manually. This is the same pattern identified in P2-5 (learn) and P2-9 (review): skills reference the Zettelkasten schema without declaring it as a dependency. The prd skill adds a third instance of this drift-prone pattern.
- **Suggested fix**: Add a `zettelkasten` key to the requires vocabulary (as recommended in P2-5) and add it to prd's `requires` list. Remove the hardcoded path reference from Step 3 prose.

### Finding P4-4: PRD vault context selection reads from three directories but `requires` only resolves `prd-index`
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/prd/SKILL.md`, Step 1; frontmatter `requires: [config, prd-index]`
- **Description**: Step 1 lists titles from `~/.xavier/prd/` (covered by `prd-index`), `~/.xavier/knowledge/repos/`, and `~/.xavier/knowledge/teams/`. The latter two directories are covered by `repo-conventions` and `team-conventions` in the requires vocabulary, but prd does not declare either in its `requires` list. This means the router does not pre-load knowledge/repos or knowledge/teams content. The skill must read these directories manually during execution, bypassing the requires resolution system. This is inconsistent with `learn` and `review` which declare these dependencies.
- **Suggested fix**: Add `repo-conventions` and `team-conventions` to prd's `requires` list: `requires: [config, prd-index, repo-conventions, team-conventions]`. Use the resolved context in Step 1 instead of manual directory reads.

### Finding P4-5: PRD `related` field populates wikilinks from Step 1 selection but does not include links discovered during codebase exploration
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/prd/SKILL.md`, Step 3 (frontmatter template, `related` field)
- **Description**: The frontmatter template says `related: [{wikilinks to vault notes selected in Step 1}]`. This only captures notes the user explicitly selected before the interview began. During Step 2 (codebase exploration and questioning), the executor may discover connections to other vault notes -- for example, a dependency note in `knowledge/repos/` or a prior PRD that was not selected in Step 1. These discovered connections are not captured in the `related` field. The Zettelkasten reference says "The `related` field is the primary mechanism for linking notes -- prefer explicit links over implicit tag-based discovery." By limiting `related` to Step 1 selections, the PRD misses links that emerge during the interview.
- **Suggested fix**: Instruct the executor to accumulate related links throughout the interview, not just from Step 1. Add to Step 3: "Include wikilinks from Step 1 selections AND any vault notes referenced or discovered during the interview."

---

### 4b — tasks

**Happy path**: User runs `/xavier tasks`. Router resolves `requires: [config, tasks-index, prd-index]`. Step 1 lists PRDs from `~/.xavier/prd/` for selection. Step 2 reads the selected PRD and auto-loads related notes (with a guardrail for 8+ links). Step 3 explores the codebase and detects backpressure commands. Step 4 identifies architectural decisions. Step 5 drafts vertical slices. Step 6 quizzes the user on the breakdown. Step 7 writes the tasks file. Step 8 stops and offers next-step options. This is a well-structured pipeline with good guardrails.

**Failure path -- no PRDs available**: Step 1 lists `.md` files in `~/.xavier/prd/`. If the directory is empty or does not exist, the `prd-index` requires key resolves to an empty list. The skill presents an empty numbered list -- there is nothing to select. The skill has no handling for this case.

**Failure path -- malformed PRD**: Step 2 reads the PRD and checks its `related` field. If the PRD has no frontmatter (was hand-written or corrupted), the `related` field is missing and there are no links to auto-load. The skill should still proceed -- the auto-load is additive, not required. However, Step 2 does not specify what happens when frontmatter is missing.

**Failure path -- quiz loop never converges**: Step 6 says "Iterate until the user approves." There is no maximum iteration count or escape hatch. If the user keeps requesting changes, the loop continues indefinitely. Unlike the loop skill (which has max iterations) and the grill skill (which has convergence detection), the tasks quiz has no bound.

**Failure path -- task file conflicts**: Step 7 writes to `~/.xavier/tasks/<filename>.md`. If a task file with the same name already exists (e.g., from a previous decomposition of the same PRD), the skill has no overwrite guard. The existing file is silently replaced.

### Finding P4-6: Tasks skill has no handling when no PRDs exist
- **Severity**: `confusion`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/tasks/SKILL.md`, Step 1 (Select PRD)
- **Description**: Step 1 says "List all `.md` files in `~/.xavier/prd/`... Present as a numbered list using AskUserQuestion." If the prd directory is empty or does not exist, the resolved `prd-index` context is empty. The skill would present an empty list to the user with no explanation. There is no guard saying "If no PRDs exist, inform the user: 'No PRDs found. Run /xavier prd to create one first.' Stop execution." The user sees an empty selection prompt and has no clear path forward.
- **Suggested fix**: Add a guard at the start of Step 1: "If the resolved `prd-index` context contains no files, tell the user: 'No PRDs found in the vault. Create one first with `/xavier prd`.' Stop execution."

### Finding P4-7: Tasks quiz loop has no iteration limit or escape hatch
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/tasks/SKILL.md`, Step 6 (Quiz the User)
- **Description**: Step 6 says "Iterate until the user approves." There is no maximum number of iterations, no convergence detection, and no "accept as-is" shortcut. If the user and executor cannot agree on the decomposition (e.g., the user keeps requesting contradictory changes), the loop runs indefinitely, consuming context window tokens. The grill skill (Step 5) has explicit convergence detection ("3 consecutive questions where the user confirms or makes only minor clarifications"). The loop skill has a max-iterations default of 10. The tasks quiz has neither mechanism.
- **Suggested fix**: Add a soft limit: "After 3 rounds of revision, present the current breakdown with a summary of changes made and ask: 'Shall we finalize this version, or continue refining?' This gives the user an explicit off-ramp without forcing premature acceptance."

### Finding P4-8: Tasks file overwrites existing file with same name without warning
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/tasks/SKILL.md`, Step 7 (Write Tasks File)
- **Description**: Step 7 writes to `~/.xavier/tasks/<filename>.md` where the filename derives from the source PRD. If the user re-decomposes the same PRD (e.g., after updating it), the tasks file already exists. The skill does not check for an existing file and has no overwrite confirmation. The previous decomposition is silently replaced. By contrast, the export skill (Step 4 point 3) explicitly checks for existing files and asks the user to confirm overwrite. The vault's git history preserves the old version, but the user receives no warning that they are about to replace an existing task list.
- **Suggested fix**: Before writing, check if the file exists. If so, ask: "A task file for this PRD already exists (created {date}). Overwrite with the new decomposition, or save as `<filename>-v2.md`?"

### Finding P4-9: Tasks `source` wikilink uses PRD filename but filename confirmation happens in prd skill, not tasks
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/tasks/SKILL.md`, Step 7 (frontmatter template, `source` field)
- **Description**: The tasks frontmatter includes `source: "[[prd/<filename>]]"` linking back to the originating PRD. The `<filename>` must match the actual PRD filename in the vault. But the tasks skill does not specify how to derive this -- it reads the PRD in Step 2 but the filename comes from the `prd-index` listing in Step 1. If the PRD was renamed after creation (manually by the user), or if the user selected a PRD by title rather than filename, the executor must track which file was selected. This is implicitly clear in the happy path but fragile: the wikilink becomes broken if the PRD file is ever moved or renamed.
- **Suggested fix**: Derive the `source` wikilink from the actual file path read in Step 2 (not the display name from Step 1). Add a note: "The source wikilink uses the PRD's current filename. If the PRD is later renamed, this link will break -- update it manually or re-run `/xavier tasks`."

### Finding P4-10: Tasks requires `tasks-index` but never uses it
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/tasks/SKILL.md`, frontmatter `requires: [config, tasks-index, prd-index]`
- **Description**: The tasks skill declares `tasks-index` in its requires list. The `tasks-index` key resolves to "List all `.md` files in `<vault>/tasks/` with titles and frontmatter." However, nowhere in the skill's steps does it reference existing task files. Step 1 lists PRDs (using `prd-index`), not tasks. The only scenario where existing tasks would be relevant is the overwrite check (Finding P4-8), which the skill does not perform. The `tasks-index` context is loaded by the router, consuming context tokens, but never read by the skill.
- **Suggested fix**: Either (a) remove `tasks-index` from the requires list since it is unused, or (b) use it in Step 1 to show which PRDs already have task decompositions (e.g., "PRDs with existing tasks: auth-middleware (created 2026-04-01)") so the user knows before selecting.

### Finding P4-11: Tasks backpressure detection duplicates loop pre-flight logic with no shared reference
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/tasks/SKILL.md`, Step 3 (backpressure detection table); `xavier/skills/loop/SKILL.md`, Step 2 point 1
- **Description**: Step 3 contains a table mapping config files to backpressure commands (package.json -> npm test, Cargo.toml -> cargo test, etc.). The loop skill's Step 2 also detects and runs backpressure commands, but its detection logic is embedded in the Shark protocol reference. The tasks skill hardcodes the detection table inline. If a new language or build system is supported, both locations must be updated independently. There is no shared reference for "how to detect backpressure commands from a project." This is similar to the Zettelkasten schema drift problem (P2-5): duplicated knowledge across skills.
- **Suggested fix**: Extract the backpressure detection table into a shared reference (e.g., `references/patterns/backpressure.md`) and have both tasks and loop reference it. Add a `backpressure` key to the requires vocabulary.

### Finding P4-12: Tasks Step 8 stop guardrail is soft -- no mechanism prevents the executor from continuing
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/tasks/SKILL.md`, Step 8 (STOP guardrail)
- **Description**: Step 8 includes a `<stop-guardrail>` tag with "You are DONE. Do not write any code." This is a prompt-level instruction with no enforcement. An eager LLM executor may interpret the user's next message (e.g., "looks good, let's go") as permission to start implementing, especially since Step 8 presents "Start immediately: run `/xavier loop`" as an option. The executor might start coding instead of properly handing off to a fresh `/xavier loop` invocation. The review skill has a similar stop boundary but it is at the natural end of the skill. Tasks explicitly offers "start implementing" as a next step, creating tension between the stop guardrail and the suggested action.
- **Suggested fix**: Strengthen the guardrail: "After presenting options, STOP responding. The user must invoke `/xavier loop` in a new conversation. Do not run loop inline, do not write code, do not explore implementation details even if the user asks." Alternatively, remove the "Start immediately" option since it undermines the clean-context recommendation.

---

### 4c — grill

**Happy path**: User runs `/xavier grill`. Router resolves `requires: [shark, adapter]`. Step 1 checks SHARK_TASK_HASH (unset -- proceed with full flow). Step 2 reads the adapter and asks the user to describe the plan. Step 3 spawns 3-5 research remoras in parallel, collects results, compiles a Research Brief, and presents it to the user. Step 4 runs the interview one question at a time, grounded in the research brief. Step 5 detects convergence (3 consecutive confirms/minor-clarifications) and writes a shared-understanding summary. This is a well-designed flow with good research-first-then-interview structure.

**Failure path -- no topic provided**: Step 2 says "Ask the user to describe the plan or design they want grilled, or read it from a file/PR if they point to one." If the user provides nothing (empty response or vague input like "my project"), the executor must decide whether to proceed with vague research queries or ask again. No minimum-input validation is specified.

**Failure path -- convergence never reached**: Step 5 defines convergence as "3 consecutive questions where the user confirms or makes only minor clarifications." If the user keeps raising new concerns or changing direction, convergence never triggers. Unlike the tasks quiz (which at least implicitly ends when the user says "approved"), the grill interview has no explicit "I'm done" escape besides the convergence heuristic. There is no maximum question count.

**Failure path -- adapter mismatch**: Step 2 says "If no adapter is wired, warn and fall back to inline execution." This is a good degradation path -- the interview works without background agents, just slower. No issue here.

**Failure path -- output location ambiguous**: The grill skill's convergence output (Step 5) says to write a shared-understanding summary, but does not specify where. It is not written to the vault. It exists only in the conversation context.

### Finding P4-13: Grill has no minimum-input validation for the plan description
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/grill/SKILL.md`, Step 2 (Pre-flight, point 2)
- **Description**: Step 2 says "Ask the user to describe the plan or design they want grilled." If the user provides a one-word answer ("authentication") or an empty response, the executor must generate research axes (Step 3) from insufficient context. The research remoras will produce broad, unfocused results, and the interview will start without a clear scope. There is no instruction to validate the input or ask for more detail before proceeding. By contrast, the prd skill explicitly asks for "a long, detailed description of the problem."
- **Suggested fix**: Add input validation: "If the user's plan description is under 2 sentences, ask for more detail: 'Can you describe the plan in more detail? What problem does it solve, what approach are you considering, and what are you unsure about?' Do not proceed to research until you have sufficient context to generate focused research axes."

### Finding P4-14: Grill convergence detection has no maximum question count or explicit exit
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/grill/SKILL.md`, Step 5 (implied convergence logic within Step 4)
- **Description**: The grill skill's interview phase (Step 4) continues until convergence is detected in Step 5: "3 consecutive questions where the user confirms or makes only minor clarifications." There is no maximum question count and no explicit "I'm done" command the user can issue to end the interview early. If the plan is complex and the user keeps providing substantive answers, the interview could consume the entire context window before convergence triggers. The user's only escape is to stop responding or close the session, losing the accumulated shared understanding.
- **Suggested fix**: Add a dual exit mechanism: (a) a maximum question count (e.g., 20 questions) with a warning at question 15, and (b) an explicit exit: "At any point, the user can say 'wrap up' to skip to the shared-understanding summary with what has been covered so far."

### Finding P4-15: Grill shared-understanding output is not persisted to the vault
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/grill/SKILL.md`, Step 5 (convergence output, implied)
- **Description**: When convergence is reached, the grill skill produces a shared-understanding summary. However, the skill does not specify where this summary is written. It is not saved to `~/.xavier/` or any vault location. The summary exists only in the conversation context and is lost when the session ends. For a planning skill whose purpose is to reach "shared understanding," losing the output when the session closes defeats the purpose. The prd skill writes to `~/.xavier/prd/`, the tasks skill writes to `~/.xavier/tasks/`, but the grill skill writes nowhere.
- **Suggested fix**: Write the shared-understanding summary to the vault. Either (a) save to `~/.xavier/knowledge/grills/<topic>.md` with Zettelkasten frontmatter, or (b) offer the user a choice: "Save this summary to the vault as a knowledge note? (It can inform future PRDs and reviews via the `related` field.)" At minimum, present the summary in a copy-pasteable format.

### Finding P4-16: Grill requires `[shark, adapter]` but does not require `config`
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/grill/SKILL.md`, frontmatter `requires: [shark, adapter]`
- **Description**: The grill skill requires `adapter`, and as identified in P1-5, the `adapter` requires key implicitly depends on `config` (it reads the adapter name from config.md). But grill does not declare `config` in its requires list. This means the router resolves `adapter` by implicitly reading config, but the executor has no explicit config context available for other purposes (e.g., reading team name or git-strategy). If the vault gate check (which fires when requires is non-empty) passes based on config.md existence, this works in practice. But the implicit dependency is undocumented and inconsistent with prd (`requires: [config, prd-index]`) and tasks (`requires: [config, tasks-index, prd-index]`) which both explicitly declare `config`.
- **Suggested fix**: Add `config` to grill's requires list: `requires: [config, shark, adapter]`. This makes the dependency explicit and gives the executor access to config context for the vault commit step.

### Finding P4-17: Grill detect-and-defer checks `SHARK_TASK_HASH` but inherits the unactivated-variable problem from P2-1
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/grill/SKILL.md`, Step 1 (Detect-and-Defer)
- **Description**: Step 1 runs `echo "$SHARK_TASK_HASH"` to check for nested execution. As identified in P2-1, `SHARK_TASK_HASH` is checked by all Shark-consuming skills but never set by any spawning mechanism. This means the detect-and-defer check in grill will always find the variable unset, and grill will always run the full flow -- even when spawned as a sub-agent by another skill. The inline "act as a simple interviewer" degradation path is dead code. Unlike learn (where detect-and-defer ordering was the problem -- P3-12), grill has the detect-and-defer correctly at Step 1 but the variable itself is never populated.
- **Suggested fix**: Same as P2-1 -- define who sets `SHARK_TASK_HASH` and how. Until that is resolved, grill's detect-and-defer is non-functional.

### Finding P4-18: Grill research remoras use `Agent()` directly, bypassing the adapter it declares in requires
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/grill/SKILL.md`, Step 3 (research remora spawn example)
- **Description**: Grill declares `adapter` in its requires list and Step 2 says "Read adapter: Use the resolved `adapter` context to know how to spawn agents." But Step 3's code example directly uses `Agent()` with Claude Code-specific parameters (`run_in_background: true`, `subagent_type: "Explore"`). The skill loads the adapter context but then ignores it, hardcoding the Claude Code Agent tool. This is the same pattern as P2-2 and P3-15. Grill is notable because it is the one skill that explicitly acknowledges the adapter in its pre-flight ("If no adapter is wired, warn and fall back to inline execution") yet still bypasses it in the actual spawning code.
- **Suggested fix**: Either update the code example to reference `adapter.spawn()` vocabulary, or acknowledge the adapter is documentation-only and remove the pretense of adapter abstraction.

---

## Phase 4 Summary

| ID | Title | Severity | Tag |
|----|-------|----------|-----|
| P4-1 | PRD does not specify how to resolve `repo` for frontmatter | `silent-failure` | `executor-facing` |
| P4-2 | PRD interview has no checkpoint or draft mechanism | `friction` | `user-facing` |
| P4-3 | PRD references Zettelkasten via prose path, not requires | `confusion` | `executor-facing` |
| P4-4 | PRD reads 3 directories but only requires `prd-index` | `friction` | `executor-facing` |
| P4-5 | PRD `related` field misses links discovered during interview | `friction` | `executor-facing` |
| P4-6 | Tasks has no handling when no PRDs exist | `confusion` | `user-facing` |
| P4-7 | Tasks quiz loop has no iteration limit or escape hatch | `friction` | `user-facing` |
| P4-8 | Tasks file overwrites existing file without warning | `silent-failure` | `user-facing` |
| P4-9 | Tasks `source` wikilink derivation is implicit and fragile | `friction` | `executor-facing` |
| P4-10 | Tasks requires `tasks-index` but never uses it | `friction` | `executor-facing` |
| P4-11 | Tasks backpressure detection duplicates loop logic with no shared reference | `confusion` | `executor-facing` |
| P4-12 | Tasks stop guardrail is soft with no enforcement | `friction` | `executor-facing` |
| P4-13 | Grill has no minimum-input validation for plan description | `friction` | `user-facing` |
| P4-14 | Grill convergence has no max question count or explicit exit | `friction` | `user-facing` |
| P4-15 | Grill shared-understanding output not persisted to vault | `silent-failure` | `user-facing` |
| P4-16 | Grill requires `adapter` but not `config` (implicit dependency) | `silent-failure` | `executor-facing` |
| P4-17 | Grill detect-and-defer inherits unactivated `SHARK_TASK_HASH` | `silent-failure` | `executor-facing` |
| P4-18 | Grill spawns agents directly despite requiring adapter | `confusion` | `executor-facing` |

**Phase 4 severity breakdown**: 5 silent-failure, 4 confusion, 9 friction
**Phase 4 tag breakdown**: 6 user-facing, 12 executor-facing

**Cumulative totals (Phase 1 + Phase 2 + Phase 3 + Phase 4)**: 61 findings — 19 silent-failure, 17 confusion, 25 friction

---

## Phase 5 — Skills Tier 3 (Maintenance)

Phase 5 traces the five maintenance skills (babysit, add-dep, deps-update, self-update, export) through happy and failure paths.

### Skill: babysit

**Happy path**: User runs `/xavier babysit 20`. Router resolves config. Skill detects repo from `git remote -v`, asks for PR number, validates via `gh pr view`, creates state file at `~/.xavier/babysit-pr/<repo>-<pr>.md`, then delegates to `/xavier loop` with 10-minute interval. Each cycle checks branch, PR state, CI status, auto-fixes lint when safe (<=10 files), investigates other failures read-only, surfaces new review comments, and logs everything.

**Failure paths traced**: PR already merged (caught at 1b), no CI configured (2d returns no checks — handled), lint fix introduces new errors (caught at 2e step 5 — falls through to 2f), rate limiting (not handled), branch mismatch (pauses correctly).

### Finding P5-1: babysit requires only `config` but reads vault dir `~/.xavier/babysit-pr/`
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/babysit/SKILL.md`, Step 1d
- **Description**: The skill creates and reads `~/.xavier/babysit-pr/` state files but does not declare any vault-directory dependency in its `requires` list. The router has no `babysit-state` key in the requires vocabulary either, so state directory creation is ad-hoc via inline `mkdir -p`. This is consistent with `loop-state/` usage in `loop`, but means the router cannot pre-validate or pre-create the directory.
- **Suggested fix**: Either add a `babysit-state` requires key or document that skills may self-create state directories as an accepted pattern.

### Finding P5-2: babysit delegates to `/xavier loop` but loop requires `shark` — implicit transitive dependency
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/babysit/SKILL.md`, Step 1e
- **Description**: babysit's `requires` is `[config]` but it delegates to `/xavier loop` which requires `[config, shark]`. When babysit hands off to loop, the router must re-resolve requires for loop. If this is a direct delegation (not a full `/xavier loop` invocation through the router), the shark context will never be loaded, silently breaking the loop's Shark protocol.
- **Suggested fix**: Clarify whether babysit triggers loop through the router (which re-resolves requires) or as an inline delegation. If inline, babysit must add `shark` to its own requires. If through the router, document the re-invocation pattern.

### Finding P5-3: babysit `git add -u` can stage unrelated changes
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/babysit/SKILL.md`, Step 2e (Lint Auto-Fix)
- **Description**: The lint auto-fix step runs `git add -u && git commit -m "fix: lint"` which stages all tracked file modifications, not just lint-fix changes. If the user has uncommitted work on the branch, it gets swept into the lint-fix commit and auto-pushed.
- **Suggested fix**: Stage only files that the lint-fix commands actually changed. Run `git diff --name-only` before and after the fix command, then `git add` only the difference.

### Finding P5-4: babysit has no GitHub API rate-limit handling
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/babysit/SKILL.md`, Step 2 (Polling Cycle)
- **Description**: Each 10-minute cycle makes multiple `gh` API calls (`gh pr view`, `gh pr checks`, `gh api .../comments`). Over 50 rounds this is 150+ API calls. If rate-limited, `gh` commands fail silently or with errors that the skill does not detect or back off from.
- **Suggested fix**: Check `gh` exit codes. On rate-limit errors (HTTP 403 with rate-limit headers), log a warning, extend the polling interval, and continue rather than treating the cycle as a no-op.

### Finding P5-5: babysit lint detection is JS-only but rule 6 is buried
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/babysit/SKILL.md`, Rule 6
- **Description**: The skill hardcodes fix-command detection to `package.json` scripts (rule 6: "JS ecosystem only (v1)"). This is stated only in the Rules section at the bottom. If a user runs babysit on a Python or Go repo, lint failures will be detected but auto-fix will silently skip because no `package.json` exists. No error or warning is shown.
- **Suggested fix**: At Step 1 (Setup), check for `package.json`. If absent, warn the user that auto-fix is unavailable for this ecosystem and lint failures will be routed to manual investigation.

### Finding P5-6: babysit commit message is non-descriptive
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/babysit/SKILL.md`, Step 2e
- **Description**: Lint-fix commits use the generic message `"fix: lint"` with no context about which checks failed or what was changed. Over multiple rounds, the git log becomes a wall of identical `fix: lint` entries.
- **Suggested fix**: Include the check name and file count in the commit message, e.g., `"fix(babysit): eslint — 3 files"`.

---

### Skill: add-dep

**Happy path**: User runs `/xavier add-dep zod`. Router resolves config and skills-index. Skill checks if `~/.xavier/skills/zod/` exists (via skills-index), uses WebSearch + WebFetch to gather docs, spawns an Agent to distill a dependency-skill reference, writes `~/.xavier/skills/zod/SKILL.md` with frontmatter.

**Failure paths traced**: package not found (WebSearch returns no results — not handled), skill already exists (asks user — handled), write permission errors (not handled), malformed package metadata (not handled).

### Finding P5-7: add-dep does not update skills-index after creating a new skill
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/add-dep/SKILL.md`, Step 4
- **Description**: The skill requires `skills-index` (a directory listing of `<vault>/skills/`) for its existence check in Step 1, but after writing a new skill file in Step 4, it never refreshes or signals that skills-index is now stale. If a subsequent skill (e.g., deps-update) runs in the same session, the cached skills-index won't include the newly created skill.
- **Suggested fix**: After Step 4, explicitly note that the skills-index has been invalidated, or re-read the directory listing to update the resolved context.

### Finding P5-8: add-dep has no fallback when WebSearch or WebFetch fails
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/add-dep/SKILL.md`, Step 2
- **Description**: Steps 2.2 and 2.3 rely on WebSearch and WebFetch to find and read package documentation. If either tool fails (network error, no results, blocked site), the skill has no fallback. The agent in Step 3 would receive no documentation context and produce a low-quality or hallucinated skill file with no warning to the user.
- **Suggested fix**: If WebSearch returns no results or WebFetch fails, warn the user and offer alternatives: provide a doc URL manually, use the package's README from `node_modules/`, or abort.

### Finding P5-9: add-dep creates dependency skills inside the skills/ directory alongside Xavier's own skills
- **Severity**: `confusion`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/add-dep/SKILL.md`, Step 4
- **Description**: Dependency skills are written to `~/.xavier/skills/<package-name>/SKILL.md`, the same directory that houses Xavier's own command skills (review, babysit, export, etc.). This means `skills-index` (which lists all directories in `<vault>/skills/`) conflates command skills with dependency-knowledge skills. The router's help listing (scan skills/ for commands) would show dependency packages as commands. self-update's `rm -rf "$XAVIER_HOME/skills/"` would delete all user-generated dependency skills.
- **Suggested fix**: Use a separate directory for dependency skills, e.g., `~/.xavier/deps/` or `~/.xavier/skills-deps/`, to keep them isolated from router command skills. Alternatively, use the `type: dependency` frontmatter field to filter them out of command listings — but self-update's `rm -rf` still destroys them.

### Finding P5-10: add-dep does not validate that the package actually exists in any registry
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/add-dep/SKILL.md`, Step 2
- **Description**: Step 2.1 checks `package.json` to see if the package is already a dependency but does not validate that the package name is a real published package. A typo like `/xavier add-dep zdo` would proceed through WebSearch, potentially find unrelated results, and create a skill file for a nonexistent package.
- **Suggested fix**: Run `npm view <package-name> name version` (or equivalent) as a validation step. If it fails, warn the user the package was not found in the registry.

---

### Skill: deps-update

**Happy path**: User runs `/xavier deps-update`. Router resolves config and skills-index. Skill reads lockfile (or falls back to package.json), compares versions against existing dependency skills' frontmatter, marks stale ones, asks user if >5 stale, regenerates by re-running add-dep Steps 2-4, reports results including orphaned skills.

**Failure paths traced**: no lockfile and no package.json (not handled), no stale deps (reports zero — handled), regeneration fails for some deps (not handled), partial update (not handled).

### Finding P5-11: deps-update has no handling when neither lockfile nor package.json exists
- **Severity**: `confusion`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/deps-update/SKILL.md`, Step 1
- **Description**: Step 1 checks for lockfiles, then falls back to `package.json`. If none exist (e.g., running from the wrong directory, or a non-JS project), the skill has no error handling. It would proceed with an empty dependency list, find nothing stale, and report "0 checked, 0 stale" — giving a false sense that everything is up to date.
- **Suggested fix**: If no lockfile and no `package.json` are found, stop with a clear error: "No lockfile or package.json found in the current directory. Run this command from a project root."

### Finding P5-12: deps-update orphan detection cannot distinguish command skills from dependency skills
- **Severity**: `confusion`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/deps-update/SKILL.md`, Step 3
- **Description**: The skill compares skills in `~/.xavier/skills/` against the lockfile and marks skills not in the lockfile as "orphaned." Since dependency skills live alongside command skills (see P5-9), every Xavier command skill (review, babysit, export, etc.) would be flagged as orphaned and suggested for removal.
- **Suggested fix**: Filter skills by `type: dependency` frontmatter before orphan detection, or use a separate directory for dependency skills.

### Finding P5-13: deps-update does not handle partial regeneration failure
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/deps-update/SKILL.md`, Step 3
- **Description**: When regenerating stale skills, the skill re-runs add-dep's Steps 2-4 for each one. If WebSearch/WebFetch fails for some packages (network issues, rate limiting), those skills may be silently left with outdated or corrupted content. The Step 4 report does not distinguish between successfully regenerated and failed regenerations.
- **Suggested fix**: Track success/failure per package during regeneration. In the report, list which packages were successfully updated and which failed, with the reason.

### Finding P5-14: deps-update confirmation threshold is static at 5
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/deps-update/SKILL.md`, Step 3
- **Description**: The skill asks for user confirmation only when there are more than 5 stale skills. For 1-5 stale skills, it regenerates without asking. Each regeneration involves WebSearch + WebFetch + Agent spawning, which has real cost and latency. Even 3-4 regenerations could take significant time and API calls.
- **Suggested fix**: Always show the list of stale packages and ask for confirmation, or make the threshold configurable via config.

---

### Skill: self-update

**Happy path**: User runs `/xavier self-update`. Router resolves config. Skill reads current version from config.md, fetches latest release tag via `gh api`, compares versions, shows update summary with release notes, asks user to confirm, downloads tarball, extracts, replaces `skills/` and `references/` directories, updates version in config, ensures vault directories, cleans up temp dir, reports success.

**Failure paths traced**: already up to date (handled at Step 3), network failure (handled at Step 6), symlink permission error (not handled), breaking changes (not handled).

### Finding P5-15: self-update `rm -rf skills/` destroys user-generated dependency skills
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/self-update/SKILL.md`, Step 8
- **Description**: Step 8 runs `rm -rf "$XAVIER_HOME/skills/"` then copies new skills from the tarball. Since add-dep writes dependency skills into the same `skills/` directory (see P5-9), every user-generated dependency skill is permanently deleted during self-update. The "Files that MUST NOT be touched" list does not mention dependency skills.
- **Suggested fix**: Before removing `skills/`, back up dependency skills (those with `type: dependency` frontmatter) to a temp location and restore them after copying new skills. Better yet, resolve P5-9 by using a separate directory for dependency skills.

### Finding P5-16: self-update has no rollback mechanism on partial failure
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/self-update/SKILL.md`, Steps 8-9
- **Description**: If the update fails partway through Step 8 (e.g., after `rm -rf skills/` but before `cp -R` completes), the vault is left in a broken state with no skills directory. There is no backup of the old skills/references before deletion and no rollback procedure.
- **Suggested fix**: Back up `skills/` and `references/` to a temp location before removal. If any subsequent step fails, restore from backup. Only delete backups after Step 12 (success).

### Finding P5-17: self-update version comparison is naive string equality
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/self-update/SKILL.md`, Step 3
- **Description**: Step 3 says "compare the current installed version against the target version" and stops if "they are equal." There is no semver-aware comparison. If the current version is `1.0.0` and the target is `0.9.0` (a downgrade), the skill would proceed with the "update." Similarly, pre-release versions like `1.0.0-beta.1` are compared as plain strings.
- **Suggested fix**: Add semver-aware comparison. If the target is older than current, warn the user this is a downgrade and ask for explicit confirmation.

### Finding P5-18: self-update `XAVIER_HOME` is not resolved consistently
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/self-update/SKILL.md`, Steps 8-10
- **Description**: The skill uses `$XAVIER_HOME` in bash commands but the router resolves XAVIER_HOME in Step 0 and stores the path internally. The skill's bash snippets reference the shell variable directly. If the environment variable is not actually exported (e.g., the router used the `~/.xavier/` default without setting the env var), all bash commands referencing `$XAVIER_HOME` will fail or use an empty string, potentially running `rm -rf /skills/`.
- **Suggested fix**: The skill should use the resolved vault path from the router context rather than referencing the shell variable. Or the router should ensure `$XAVIER_HOME` is exported before skill execution.

### Finding P5-19: self-update does not verify tarball integrity
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/self-update/SKILL.md`, Steps 6-7
- **Description**: The tarball is downloaded via `curl` and extracted without any checksum or signature verification. A corrupted download or MITM attack could install malicious or broken skill files.
- **Suggested fix**: If the GitHub release includes a checksum file, verify the tarball against it after download. At minimum, verify the tarball extracts successfully and contains the expected directory structure before proceeding to replacement.

---

### Skill: export

**Happy path**: User runs `/xavier export prd/my-feature`. Router resolves config. Skill reads `export-vault-path` from config, resolves the source file relative to `~/.xavier/`, reads content, adapts wikilinks (rewrites exported ones to `x-inbox/x-<name>`, strips unexported ones), creates `x-inbox/` directory, writes to `{export-vault-path}/x-inbox/x-my-feature.md`, tells user the file location.

**Failure paths traced**: export path missing from config (asks user — handled), wikilinks can't resolve (stripped to plain text — handled), target already exists (asks for overwrite — handled), partial export (not handled).

### Finding P5-20: export excludes `personas/` and `adapters/` from listing but they live under `references/`
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `xavier/skills/export/SKILL.md`, Step 2
- **Description**: Step 2 excludes `personas/`, `adapters/`, `loop-state/`, `review-state/`, `skills/` from the exportable directories listing. But personas and adapters actually live at `references/personas/` and `references/adapters/`. The exclusion pattern would need to match `references/personas/`, not just `personas/`. Meanwhile, `references/` as a whole is not excluded, so users could navigate into it and export internal reference files.
- **Suggested fix**: Exclude `references/` entirely from the exportable list, or use full relative paths in the exclusion list (`references/personas/`, `references/adapters/`).

### Finding P5-21: export wikilink adaptation silently drops link context
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/export/SKILL.md`, Step 3
- **Description**: When a wikilink target has not been exported, the link is stripped to plain text (e.g., `[[my-note]]` becomes `my-note`). The user's export has no indication that these were originally links. If the user later exports the linked note, the earlier export is not updated — the plain-text reference remains stale.
- **Suggested fix**: Preserve stripped wikilinks with a visual marker (e.g., `*my-note*` or `my-note [not exported]`) so the user knows these were links. Optionally, after export, list all unresolved wikilinks and suggest additional notes to export.

### Finding P5-22: export filename collision when different paths share a filename
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/export/SKILL.md`, Step 4
- **Description**: The destination filename is derived from the source filename only, ignoring the path: `prd/my-feature.md` and `tasks/my-feature.md` both become `x-my-feature.md`. Exporting both would silently overwrite the first with the second (or trigger the overwrite prompt without explaining they are different source files).
- **Suggested fix**: Include the source directory in the export filename (e.g., `x-prd-my-feature.md`, `x-tasks-my-feature.md`) or detect the collision and warn the user.

### Finding P5-23: export does not handle frontmatter adaptation
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/export/SKILL.md`, Step 3
- **Description**: Step 3 says "Preserve all other Obsidian-flavored markdown: ... frontmatter." Xavier vault notes use Xavier-specific frontmatter fields (e.g., `type`, `source`, `uid`) from the Zettelkasten schema. These fields are meaningless in the user's personal vault and could interfere with their Obsidian setup (e.g., Dataview queries, templates).
- **Suggested fix**: Add an option to strip or namespace Xavier-specific frontmatter fields during export, or add an `x-source` field pointing back to the Xavier vault origin.

### Finding P5-24: export `export-show-diff` default false means overwrites happen without visibility
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `xavier/skills/export/SKILL.md`, Step 4
- **Description**: When a destination file already exists, the skill asks the user to confirm overwrite. But with `export-show-diff: false` (the default), the user has no way to see what changed before deciding. They must blindly choose "Overwrite" or "Skip."
- **Suggested fix**: Default `export-show-diff` to `true`, or at minimum show a brief summary (e.g., "Source was modified on {date}, existing export is from {date}") to help the user decide.

---

### Phase 5 Summary Table

| ID | Title | Severity | Tag |
|----|-------|----------|-----|
| P5-1 | babysit reads vault dir without declaring requires | `friction` | `executor-facing` |
| P5-2 | babysit delegates to loop without transitive shark dependency | `confusion` | `executor-facing` |
| P5-3 | babysit `git add -u` can stage unrelated changes | `silent-failure` | `user-facing` |
| P5-4 | babysit has no GitHub API rate-limit handling | `silent-failure` | `user-facing` |
| P5-5 | babysit lint detection is JS-only but rule is buried | `friction` | `user-facing` |
| P5-6 | babysit commit message is non-descriptive | `friction` | `user-facing` |
| P5-7 | add-dep does not update skills-index after creating a skill | `silent-failure` | `executor-facing` |
| P5-8 | add-dep has no fallback when WebSearch/WebFetch fails | `silent-failure` | `user-facing` |
| P5-9 | add-dep creates dependency skills alongside command skills | `confusion` | `user-facing` |
| P5-10 | add-dep does not validate package exists in registry | `friction` | `user-facing` |
| P5-11 | deps-update has no handling when no lockfile or package.json exists | `confusion` | `user-facing` |
| P5-12 | deps-update orphan detection conflates command and dependency skills | `confusion` | `user-facing` |
| P5-13 | deps-update does not handle partial regeneration failure | `silent-failure` | `user-facing` |
| P5-14 | deps-update confirmation threshold is static at 5 | `friction` | `user-facing` |
| P5-15 | self-update destroys user-generated dependency skills | `silent-failure` | `user-facing` |
| P5-16 | self-update has no rollback on partial failure | `silent-failure` | `user-facing` |
| P5-17 | self-update version comparison is naive string equality | `friction` | `executor-facing` |
| P5-18 | self-update XAVIER_HOME not resolved consistently | `confusion` | `executor-facing` |
| P5-19 | self-update does not verify tarball integrity | `friction` | `user-facing` |
| P5-20 | export exclusion paths don't match actual directory structure | `friction` | `executor-facing` |
| P5-21 | export wikilink adaptation silently drops link context | `friction` | `user-facing` |
| P5-22 | export filename collision across source directories | `silent-failure` | `user-facing` |
| P5-23 | export does not handle frontmatter adaptation | `friction` | `user-facing` |
| P5-24 | export-show-diff defaults to false, overwrites lack visibility | `friction` | `user-facing` |

**Phase 5 severity breakdown**: 8 silent-failure, 5 confusion, 11 friction
**Phase 5 tag breakdown**: 16 user-facing, 8 executor-facing

**Cumulative totals (Phase 1 + Phase 2 + Phase 3 + Phase 4 + Phase 5)**: 85 findings — 27 silent-failure, 22 confusion, 36 friction

---

# Phase 6 — Skills Tier 4 (Destructive)

Phase 6 audits the two destructive skills (`remove-dep`, `uninstall`) with special focus on confirmation prompts, reversibility, partial-failure handling, and accidental data destruction.

---

## remove-dep

### Happy path

1. User runs `/xavier remove-dep express`.
2. Router resolves `XAVIER_HOME`, loads `skills-index` (lists `~/.xavier/skills/`), loads `config`.
3. Step 1 — Validate: `express` was provided, `~/.xavier/skills/express/` exists. Passes.
4. Step 2 — Remove: Deletes `~/.xavier/skills/express/`. Tells the user it was removed.

### Failure path: package not provided

User runs `/xavier remove-dep` with no argument. Step 1 asks the user for the package name. Reasonable.

### Failure path: skill doesn't exist

User runs `/xavier remove-dep foo`. Step 1 checks `~/.xavier/skills/foo/`, finds it missing, lists available dependency-skills. Reasonable.

### Failure path: user accidentally targets a command skill

User runs `/xavier remove-dep review`. The `skills-index` context lists ALL directories in `~/.xavier/skills/`, which includes both dependency skills (e.g., `express`) and command-skill symlinks (e.g., `review`, `setup`, `grill`). The skill has no check for `type: dependency` in the target's frontmatter. Step 1 would confirm the directory exists. Step 2 would delete it — destroying the `review` symlink (or, worse, the actual directory if symlinks were not set up correctly).

### Failure path: user cancels — no cancellation possible

There is no confirmation prompt at any point. The skill goes straight from validation to deletion.

### Failure path: partial deletion

Step 2 is a single directory delete. No explicit error handling is defined for permission errors or partial removal.

---

### Finding P6-1: remove-dep has no confirmation prompt before deletion
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/remove-dep/SKILL.md`, Step 2
- **Description**: The skill validates the target exists and immediately deletes it. There is no confirmation prompt, no "are you sure?", and no preview of what will be deleted. For a destructive, irreversible action, this is a significant safeguard gap. A typo or tab-completion accident could silently destroy a dependency skill.
- **Suggested fix**: Add an explicit confirmation step between validation and deletion: show the skill name, its `type`, `version`, and `created` date from frontmatter, then ask for yes/no confirmation via `AskUserQuestion`.

### Finding P6-2: remove-dep does not distinguish dependency skills from command skills
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/remove-dep/SKILL.md`, Step 1
- **Description**: The `skills-index` context lists ALL directories in `~/.xavier/skills/`, which includes both dependency skills (`type: dependency` in frontmatter, e.g., `express`) and command-skill symlinks (e.g., `review`, `setup`, `grill`, `uninstall`). The remove-dep skill only checks whether the directory exists — it never reads the target's frontmatter to verify `type: dependency`. A user running `/xavier remove-dep review` would delete the `review` command skill, breaking Xavier's core functionality.
- **Suggested fix**: In Step 1, after confirming the directory exists, read the target's `SKILL.md` frontmatter and verify `type: dependency`. If the type is not `dependency` (or has no type field), refuse the deletion with a clear message: "'{name}' is a command skill, not a dependency skill. Use `/xavier uninstall` to remove Xavier entirely."

### Finding P6-3: remove-dep "list available dependency-skills" shows command skills too
- **Severity**: `confusion`
- **Tag**: `user-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/remove-dep/SKILL.md`, Step 1
- **Description**: When the target skill doesn't exist, the instruction says "list available dependency-skills." However, the `skills-index` context is an unfiltered directory listing of `~/.xavier/skills/`, which includes command skills like `review`, `setup`, `grill`, etc. The user sees these alongside actual dependency skills and may be confused about what can be removed.
- **Suggested fix**: Filter the list to only show entries whose `SKILL.md` contains `type: dependency` in frontmatter. Alternatively, label each entry with its type.

### Finding P6-4: remove-dep deletion is irreversible with no backup mechanism
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/remove-dep/SKILL.md`, Step 2
- **Description**: The skill deletes the directory without any backup. Dependency skills are generated by an agent (via `add-dep`) that fetches documentation, runs web searches, and distills content — this process takes time and network calls. Once deleted, the only recovery is to re-run `/xavier add-dep` from scratch. There is no trash/archive mechanism.
- **Suggested fix**: Either (a) move the directory to a `.xavier/trash/` folder instead of deleting it, with a note about when it can be permanently purged, or (b) since the vault is a git repo, commit before deletion so the user can recover via `git checkout`.

### Finding P6-5: remove-dep has no error handling for deletion failure
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/remove-dep/SKILL.md`, Step 2
- **Description**: Step 2 says "Delete the directory" with no guidance on what to do if the deletion fails (permissions, file locks, etc.). The executor is left to improvise. There is also no post-deletion verification that the directory is actually gone.
- **Suggested fix**: Add explicit error handling: "If deletion fails, report the error to the user and do not claim the skill was removed. Suggest checking file permissions."

---

## uninstall

### Happy path

1. User runs `/xavier uninstall`.
2. Step 1 — Vault Deletion: `~/.xavier/` exists. The skill warns the user about what will be lost (review history, dependency-skills, knowledge notes, personas, git history) and states deletion is permanent. Asks for explicit yes/no via `AskUserQuestion`.
3. User confirms. `~/.xavier/` is deleted recursively.
4. Step 2 — Remove Symlinks: Checks and removes `~/.agents/skills/xavier/` and `~/.claude/commands/xavier.md` independently.
5. Step 3 — Summary: Prints what was removed and what was not found.

This is the best-structured destructive skill in Xavier. It has a confirmation prompt, clear warnings, and a summary. However, several issues remain.

### Failure path: vault doesn't exist

Step 1 notes the vault was not found and continues to Step 2. Symlinks are still removed. Reasonable behavior.

### Failure path: user declines

Step 1 explicitly says "Abort the entire uninstall — do NOT proceed to symlink removal." This is correct and well-handled.

### Failure path: symlink already removed

Step 2 checks each symlink independently and notes if not found. Step 3 reports the status. Reasonable.

### Failure path: partial cleanup — vault deleted but symlink removal fails

Step 2 has no error handling for failed removal. If the vault is deleted but symlink removal fails (permissions, etc.), the user is left with dangling symlinks pointing to a deleted vault. The summary step would still report "removed" since the instructions don't distinguish between "attempted removal" and "successful removal."

### Failure path: partial cleanup — one symlink removed, second fails

Step 2 processes symlinks independently, which is good. But there is no error handling per-symlink, so a failure on the second symlink is not surfaced.

---

### Finding P6-6: uninstall has no backup/export prompt before vault deletion
- **Severity**: `friction`
- **Tag**: `user-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/uninstall/SKILL.md`, Step 1
- **Description**: The skill warns that deletion is permanent but does not offer to back up or export the vault first. Xavier has an `/xavier export` skill that could be suggested before deletion. Users who have accumulated review history, dependency skills, and knowledge notes over time may not realize they could export first.
- **Suggested fix**: Before the confirmation prompt, mention: "You can run `/xavier export` first to save your vault contents. Would you like to proceed with uninstall?"

### Finding P6-7: uninstall has no error handling for partial failure
- **Severity**: `silent-failure`
- **Tag**: `executor-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/uninstall/SKILL.md`, Steps 1-2
- **Description**: If `rm -rf ~/.xavier/` succeeds but symlink removal fails (permissions, non-standard filesystem, etc.), the user is left in a broken state with dangling symlinks. The summary step has no mechanism to distinguish "successfully removed" from "attempted but failed." Similarly, if vault deletion itself partially fails (some files locked), there is no guidance.
- **Suggested fix**: Wrap each removal in explicit error checking. If any step fails, report the specific error in the summary. Consider reversing the order (remove symlinks first, vault last) so that a failure on the less-critical symlink step does not leave the vault already destroyed.

### Finding P6-8: uninstall does not handle the case where symlinks are regular directories
- **Severity**: `confusion`
- **Tag**: `executor-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/uninstall/SKILL.md`, Step 2
- **Description**: Step 2 says to remove each path "if it exists (file, symlink, or directory)." This is thorough in detection but does not differentiate between a symlink (safe to remove, points to Xavier) and a regular directory that happens to have the same name (may contain user data unrelated to Xavier). An `~/.agents/skills/xavier/` that is a real directory (not a symlink) could contain files the user placed there manually.
- **Suggested fix**: Check if the path is a symlink first. If it is a symlink, remove it. If it is a regular directory, warn the user: "This path is a regular directory, not a symlink. It may contain files not managed by Xavier. Remove anyway?"

### Finding P6-9: uninstall requires empty array but does not load config
- **Severity**: `friction`
- **Tag**: `executor-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/uninstall/SKILL.md`, frontmatter line 3
- **Description**: The `requires` array is empty (`[]`), meaning the router does not resolve `config` or `skills-index` before running uninstall. This means `XAVIER_HOME` is not explicitly resolved via the standard config mechanism. The skill hardcodes `~/.xavier/` throughout. If a user has a non-default vault location (configured in `config`), uninstall would target the wrong directory — or miss it entirely.
- **Suggested fix**: Add `config` to the `requires` array and use the resolved `XAVIER_HOME` path instead of hardcoding `~/.xavier/`.

### Finding P6-10: uninstall removal order risks data loss on partial failure
- **Severity**: `silent-failure`
- **Tag**: `user-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/uninstall/SKILL.md`, Steps 1-2
- **Description**: The skill deletes the vault (the largest, most valuable data) first, then removes symlinks. If the process is interrupted after vault deletion but before symlink removal, the user loses their data AND has broken symlinks. The safer order is to remove symlinks first (low-risk, easily re-created) and delete the vault last (high-risk, irreversible).
- **Suggested fix**: Reorder: Step 1 = remove symlinks, Step 2 = delete vault (with confirmation). This way, if the process fails after symlink removal, the vault is still intact and the user can re-run setup to restore symlinks.

### Finding P6-11: uninstall summary template does not account for errors
- **Severity**: `confusion`
- **Tag**: `user-facing`
- **Location**: `/Users/atila.fassina/Developer/xavier/xavier/skills/uninstall/SKILL.md`, Step 3
- **Description**: The summary template only has two states per item: "removed" or "not found." There is no state for "removal failed" or "partially removed." If an `rm` command fails due to permissions, the summary would either incorrectly report "removed" (misleading) or the executor would have to improvise a non-templated response.
- **Suggested fix**: Add a third state to the template: "failed — {reason}". E.g., `~/.xavier/ — failed — permission denied`.

---

## Cross-Cutting Destructive-Action Safeguard Audit

| Safeguard | remove-dep | uninstall |
|---|---|---|
| Confirmation prompt | **Missing** | Present (yes/no via AskUserQuestion) |
| Warning about consequences | **Missing** | Present (lists what will be lost) |
| Backup/export suggestion | **Missing** | **Missing** |
| Reversibility mechanism | **None** (no trash, no git commit) | **None** (permanent deletion) |
| Error handling on failure | **Missing** | **Missing** |
| Partial-failure recovery | **Not addressed** | **Not addressed** |
| Type-safety (dep vs command) | **Missing** (can delete command skills) | N/A (deletes everything) |
| Non-default vault path support | Via config (in requires) | **Hardcoded** `~/.xavier/` |

---

## Phase 6 Summary Table

| ID | Description | Severity | Tag |
|---|---|---|---|
| P6-1 | remove-dep has no confirmation prompt before deletion | `silent-failure` | `user-facing` |
| P6-2 | remove-dep does not distinguish dependency skills from command skills | `silent-failure` | `user-facing` |
| P6-3 | remove-dep "list available" shows command skills alongside dependency skills | `confusion` | `user-facing` |
| P6-4 | remove-dep deletion is irreversible with no backup mechanism | `silent-failure` | `user-facing` |
| P6-5 | remove-dep has no error handling for deletion failure | `silent-failure` | `executor-facing` |
| P6-6 | uninstall has no backup/export prompt before vault deletion | `friction` | `user-facing` |
| P6-7 | uninstall has no error handling for partial failure | `silent-failure` | `executor-facing` |
| P6-8 | uninstall does not differentiate symlinks from regular directories | `confusion` | `executor-facing` |
| P6-9 | uninstall does not load config, hardcodes vault path | `friction` | `executor-facing` |
| P6-10 | uninstall removal order risks data loss on partial failure | `silent-failure` | `user-facing` |
| P6-11 | uninstall summary template has no "failed" state | `confusion` | `user-facing` |

**Phase 6 severity breakdown**: 6 silent-failure, 3 confusion, 2 friction
**Phase 6 tag breakdown**: 7 user-facing, 4 executor-facing

**Cumulative totals (Phase 1 + Phase 2 + Phase 3 + Phase 4 + Phase 5 + Phase 6)**: 96 findings — 33 silent-failure, 25 confusion, 38 friction
