---
name: review
requires: [shark, personas, adapter, recurring-patterns, repo-conventions, team-conventions]
---

# Review

Run a Shark-pattern code review on the current diff.

## Step 1: Detect-and-Defer

Follow the detect-and-defer protocol from the Shark reference. Check `SHARK_TASK_HASH`:

```bash
echo "$SHARK_TASK_HASH"
```

- **If set** (non-empty): Xavier is running inside an outer Shark loop. Do NOT start a new Shark flow. Instead, act as a simple reviewer — read the diff, apply the correctness persona, and return findings directly to the caller. Skip Steps 2-7.
- **If unset** (empty): Xavier is the top-level orchestrator. Proceed with the full Shark flow.

## Step 2: Pre-flight

1. **Read adapter**: Use the resolved `adapter` context to know how to spawn agents. If no adapter is wired, warn and fall back to inline execution (no background agents).
2. **Detect the diff**: Run `git diff` (unstaged) and `git diff --staged` (staged). Combine them. If both are empty, tell the user there are no changes to review and stop.

## Step 3: Load Vault Context

Gather context that reviewers need from the resolved `requires`:

1. **Team conventions**: Use the resolved `team-conventions` context — files from `~/.xavier/knowledge/teams/` matching the current repo or team.
2. **Recurring patterns** (active learning): Use the resolved `recurring-patterns` context. These are patterns extracted from the 10 most recent reviews for the current repo. Each pattern includes: **category** (correctness / security / performance), **one-line description**, and **recurrence count**.
   - If fewer than 2 reviews exist for this repo, the recurring patterns context will be empty — omit it from reviewer prompts.
3. **Repo-level persona overrides**: Check if `.xavier/personas/` exists in the current repo. If so, those personas override the global ones from the resolved `personas` context.

Compile team conventions into a **context block** (max 500 words) that will be prepended to the reviewer prompt. The recurring patterns are injected separately in Step 4.

## Step 4: Spawn Reviewer Remoras (Panel)

Load all three personas from the resolved `personas` context (or repo overrides if present):

1. `correctness.md`
2. `security.md`
3. `performance.md`

Spawn **3 reviewer agents concurrently** via the runtime adapter. All three must be spawned in a **single message** with parallel tool calls using `run_in_background: true`.

The reviewer prompt includes a `## Recurring Patterns` section between the context block and the diff. This section is **only included if patterns were extracted** (i.e., 2+ reviews existed and patterns were found):

```
// Spawn all 3 in ONE message — parallel background agents
Agent(
  prompt: "You are a code reviewer...\n## Persona\n{persona.md}\n## Context\n{context block}\n## Recurring Patterns\n{patterns, or omit this section entirely}\n## Diff\n{diff}",
  description: "xavier correctness",
  run_in_background: true
)
Agent(
  prompt: "You are a code reviewer...\n## Persona\n{persona.md}\n## Context\n{context block}\n## Recurring Patterns\n{patterns, or omit this section entirely}\n## Diff\n{diff}",
  description: "xavier security",
  run_in_background: true
)
Agent(
  prompt: "You are a code reviewer...\n## Persona\n{persona.md}\n## Context\n{context block}\n## Recurring Patterns\n{patterns, or omit this section entirely}\n## Diff\n{diff}",
  description: "xavier performance",
  run_in_background: true
)
```

Each reviewer receives the same diff, context block, and recurring patterns, but reviews through the lens of their persona only. Reviewers should pay extra attention to recurring patterns — these represent issues that keep coming back.

## Step 5: Pilot Fish (Incremental Aggregation)

The pilot fish aggregates findings as each reviewer completes. It does NOT wait for all reviewers before starting.

**As each reviewer completes**, update the user with progress:
- "Reviewer 1/3 complete (correctness)..."
- "Reviewer 2/3 complete (security)..."
- "Reviewer 3/3 complete (performance)..."

**After all reviewers have reported**, synthesize:

1. **Categorize** all findings by type: correctness, security, performance
2. **Deduplicate**: if two reviewers flag the same line/issue, merge into a single finding and note which reviewers flagged it
3. **Rank by severity**: critical > high > major > medium > minor > low (normalize across persona severity scales)
4. **Determine final verdict**: the most severe individual verdict wins
   - If ANY reviewer says **rethink** -> final verdict is **rethink**
   - If ANY reviewer says **request changes** (and none say rethink) -> **request changes**
   - If ALL reviewers say **approve** -> **approve**

## Step 6: Deliver Verdict

Present the synthesized review to the user:

1. Show the final verdict: **approve**, **request changes**, or **rethink**
2. Show per-reviewer verdicts: `correctness: approve | security: request changes | performance: approve`
3. List findings grouped by severity (critical > major > minor), with category tags
4. Show the total finding count and breakdown by category
5. Highlight any findings flagged by multiple reviewers (high confidence)

## Step 7: Write Review Note

Write a review note to `~/.xavier/knowledge/reviews/` with the following format:

**Filename**: `{repo-name}_{YYYY-MM-DD}_{short-hash}.md` where `short-hash` is the first 7 chars of `git rev-parse HEAD`.

```markdown
---
repo: {current repo name}
module: {most-changed directory in the diff}
type: review
verdict: {approve | request-changes | rethink}
finding-categories: [{list of categories found, e.g. correctness, security, performance}]
recurring: [{findings that appeared in past reviews of this repo}]
tags: [{inferred from findings}]
related: []
created: {ISO date}
updated: {ISO date}
---

# Review: {repo-name} ({short date})

## Verdict: {final verdict}

| Reviewer | Verdict | Findings |
|----------|---------|----------|
| correctness | {verdict} | {count} |
| security | {verdict} | {count} |
| performance | {verdict} | {count} |

## Findings

{synthesized, deduplicated findings grouped by severity — each tagged with [correctness], [security], or [performance]}

## Cross-Reviewer Findings

{findings flagged by multiple reviewers — higher confidence}

## Context

- **Diff scope**: {number of files changed, insertions, deletions}
- **Reviewers**: correctness, security, performance
```

## Step 8: Write Shark State

Write a state file to `~/.xavier/review-state/{repo-name}.md`:

```markdown
---
repo: {repo-name}
last-review: {ISO date}
verdict: {verdict}
reviewers: [correctness, security, performance]
---

# Shark State: {repo-name}

Last review completed at {ISO datetime}.
Verdict: {verdict}
Findings: {count} ({critical}/{major}/{minor})
```
