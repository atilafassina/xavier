---
name: investigate
description: Investigate a bug, behavior, or issue by spawning hypothesis-driven remoras across multiple investigation axes.
requires: [shark, adapter, repo-conventions:optional, recurring-patterns:optional]
---

# Investigate

`/xavier investigate <symptom> [--file <path>] [--test <name>]`

Investigate a bug, behavior, or issue by spawning hypothesis-driven remoras across multiple investigation axes. Produces a ranked diagnosis presented inline and saved to the vault.

## Step 1: Detect-and-Defer

Follow the detect-and-defer protocol from the Shark reference:

```bash
echo "$SHARK_TASK_HASH"
```

- **If set** (non-empty): act as a simple inline investigator — do the investigation inline, skip Shark orchestration.
- **If unset** (empty): proceed as top-level orchestrator with the full flow below.

## Step 2: Parse Input

1. Extract `--file <path>` and `--test <name>` flags if present. The remainder is the symptom description.
2. If no symptom is provided, ask the user: "Describe the bug, behavior, or issue you want to investigate."

## Step 3: Check Prior Investigations

Check for existing investigation notes that may relate to this symptom:

1. Determine the current repo name from `git rev-parse --show-toplevel` (basename).
2. Glob `~/.xavier/investigations/<repo>_*.md` for notes with matching repo prefix.
3. If matches are found, present a short list (date + symptom from frontmatter) and ask via `AskUserQuestion`: "Related to any of these, or new investigation?"
   - **Related**: read the prior note. Its content becomes additional context for each remora prompt.
   - **New**: proceed without prior context.
4. If no matches are found, proceed normally.

## Step 4: Normalize Symptom

Structure the free-text symptom into four sections:

- **What's broken**: the observable behavior or error
- **Where it manifests**: file, module, endpoint, or UI component
- **When it started**: if known (otherwise "unknown")
- **Entry point**: from `--file` or `--test` flag if provided (otherwise "none specified")

Present the normalized symptom and confirm with the user via `AskUserQuestion` before proceeding: "Does this capture the issue correctly? Edit or confirm."

## Step 5: Generate Investigation Axes

Produce 5 fixed axes plus 1-2 dynamic axes based on the symptom.

**Fixed axes (always spawned):**

1. **Code path tracing** — follow execution from symptom to inputs. Start from entry point if `--file`/`--test` provided.
2. **Recent changes** — git log/blame on affected area, looking for regression-introducing commits.
3. **Dependency boundaries** — check integration points, external calls, API contracts near symptom.
4. **Test coverage** — what tests exist for this area, are any failing, what gaps would have caught this.
5. **Error pattern matching** — search for similar error handling, known workarounds, prior occurrences.

**Dynamic axes (1-2, generated per symptom):**

Classify the symptom and add specialized axes. Examples:

- Race condition or timing issue -> **Concurrency analysis** (locks, async flows, shared state)
- Rendering or UI bug -> **State flow analysis** (component state, re-renders, prop drilling)
- Data corruption or mismatch -> **Schema drift analysis** (migrations, type definitions, serialization)
- Permission or auth error -> **Auth chain analysis** (middleware, tokens, role checks)
- Performance degradation -> **Hot path analysis** (profiling targets, N+1 queries, memory leaks)
- Configuration-related -> **Config resolution analysis** (env vars, defaults, override precedence)

## Step 6: Spawn Investigation Remoras

Spawn one remora per investigation axis via adapter `collect()` — all in a **single message** with parallel tool calls using `run_in_background: true`.

**Remora prompt template** (adapt per axis):

```
Export SHARK_TASK_HASH={hash} before starting work.

Investigate the following axis for a bug in repo "{repo}" (root: {cwd}):

**Axis**: {axis name}
**Instructions**: {axis-specific instructions}

**Symptom**:
- What's broken: {what's broken}
- Where it manifests: {where}
- When it started: {when}
- Entry point: {entry point}

{if entry point provided: "**Entry point**: Start your investigation from `{file or test name}`."}
{if repo conventions resolved: "**Repo conventions**: {conventions summary}"}
{if recurring patterns resolved: "**Known problem areas**: {recurring patterns summary}"}
{if prior investigation: "**Prior investigation found**: {prior findings}. Focus on what's new or was missed."}

Return a structured report with exactly three sections:

### Findings
What you observed in the codebase (facts only).

### Hypothesis
Your best guess at causation based on those findings.

### Evidence Strength
One of: **strong** (direct evidence linking symptom to cause), **moderate** (circumstantial evidence or partial match), **weak** (speculative, based on patterns rather than direct observation).

Constraints:
- Stay under 500 words
- Do NOT attempt to fix the bug
- Do NOT spawn sub-agents
```

**Subagent types per axis:**

- `subagent_type: "Explore"` for: code-path-tracing, test-coverage
- `subagent_type: "general-purpose"` for: recent-changes, dependency-boundaries, error-pattern-matching, and all dynamic axes

## Step 7: Collect and Synthesize

As each remora completes, record its findings. Once all have reported, the shark synthesizes:

1. **Rank hypotheses** by evidence strength: strong > moderate > weak
2. **Group related hypotheses** that point to the same root cause
3. **Identify corroborating evidence** across remoras (multiple axes supporting the same conclusion)
4. **Produce suggested next steps** for the top hypothesis — concrete and actionable: "run this test", "add a log here", "check this config"

## Step 8: Present Diagnosis

Present the unified diagnosis inline in the conversation:

1. **Top Hypothesis** — the most likely root cause with supporting evidence
2. **Ranked Hypotheses** — all hypotheses ordered by evidence strength, grouped where they converge
3. **Corroborating Evidence** — cross-remora findings that reinforce the top hypothesis
4. **Suggested Next Steps** — concrete actions to confirm or disprove the top hypothesis

## Step 9: Save to Vault

1. **Create directory if needed**: `mkdir -p ~/.xavier/investigations/` if it doesn't exist.
2. **Derive filename**: `<repo>_<date>_<slug>.md` where slug is a kebab-case short summary derived from the symptom. Strip all `/`, `\`, and `..` sequences from the result to prevent path traversal.
3. **Confirm filename** with the user via `AskUserQuestion` before writing.
4. **Write the note** to `~/.xavier/investigations/<filename>` with Zettelkasten frontmatter:

```yaml
---
repo: {current repo name}
type: investigation
created: {ISO date}
updated: {ISO date}
tags:
  - investigation
  - {symptom-derived tags}
related:
  - "[[investigations/prior-note-if-building-on]]"
symptom: "{normalized one-line symptom summary}"
verdict: "{top hypothesis one-liner}"
---
```

Then the diagnosis body:

```markdown
## Symptom

- **What's broken**: {observable behavior or error}
- **Where it manifests**: {file, module, endpoint, or UI component}
- **When it started**: {if known}
- **Entry point**: {from flags if provided}

## Diagnosis

### 1. {Top hypothesis} — {Evidence Strength}

**Findings**: {what was observed}
**Hypothesis**: {causal explanation}
**Corroborating evidence**: {cross-remora evidence if any}

### 2. {Second hypothesis} — {Evidence Strength}

**Findings**: {what was observed}
**Hypothesis**: {causal explanation}
**Corroborating evidence**: {cross-remora evidence if any}

## Suggested Next Steps

- {concrete action to confirm/disprove top hypothesis}
- {specific test to run, log to add, or config to check}

## Investigation Axes

{which axes were run and which remoras contributed to each hypothesis}
```

Tell the user the investigation note was saved and remind them they can export it with `/xavier export investigations/<filename>`.
