---
name: review
requires: [shark, personas, adapter, recurring-patterns, repo-conventions, team-conventions]
---

# Review

Run a Shark-pattern code review on the current diff.

## Step 0: Pre-flight

Check that the personas directory is non-empty — at least one of `~/.xavier/personas/` (global) or `.xavier/personas/` (repo-local) must contain `.md` files. If both are empty or missing, print: "Error: no review personas found. Ensure ~/.xavier/personas/ contains correctness.md, security.md, and performance.md." and stop.

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
3. **Check debate availability**: Run `which agent`. If it exits 0, set `debate_available = true`. If it exits non-zero, set `debate_available = false`. No error or warning is shown — this is a normal configuration state.

## Step 3: Load Vault Context

Gather context that reviewers need from the resolved `requires`:

1. **Team conventions**: Use the resolved `team-conventions` context — files from `~/.xavier/knowledge/teams/` matching the current repo or team.
2. **Recurring patterns** (active learning): Use the resolved `recurring-patterns` context. These are patterns extracted from the 10 most recent reviews for the current repo. Each pattern includes: **category** (correctness / security / performance), **one-line description**, and **recurrence count**.
   - If fewer than 2 reviews exist for this repo, the recurring patterns context will be empty — omit it from reviewer prompts.
3. **Repo-level persona overrides**: Check if `.xavier/personas/` exists in the current repo. If so, those personas override the global ones from the resolved `personas` context.

Compile team conventions into a raw **context block** (max 500 words total across all personas). The recurring patterns are filtered and injected separately in Step 4.

4. **Filter context per persona**: Before passing context to reviewers, split the loaded team conventions and recurring patterns into per-persona buckets. This filtering applies to **both** the debate (multi-model) path and the Claude-only fallback path — it runs once during context loading, before any agents are spawned.

   **Recurring patterns** are filtered by their `category` field (each pattern already carries one of: correctness, security, performance):
   - **correctness** persona receives only patterns with `category: correctness`
   - **security** persona receives only patterns with `category: security`
   - **performance** persona receives only patterns with `category: performance`

   **Team conventions** are filtered by keyword matching against the convention text. Use these explicit keyword lists:

   | Domain | Keywords |
   |--------|----------|
   | **security** | `auth`, `authentication`, `authorization`, `injection`, `secrets`, `token`, `csrf`, `xss`, `cors`, `encryption`, `tls`, `ssl`, `certificate`, `vulnerability`, `sanitize`, `escape`, `permission`, `rbac`, `acl`, `oauth` |
   | **performance** | `performance`, `caching`, `cache`, `latency`, `throughput`, `memory`, `cpu`, `optimization`, `batch`, `lazy`, `eager`, `pagination`, `index`, `query plan`, `n+1`, `connection pool`, `rate limit`, `debounce`, `throttle` |
   | **correctness** | `testing`, `test`, `error handling`, `error-handling`, `types`, `typing`, `validation`, `assertion`, `null`, `undefined`, `exception`, `boundary`, `edge case`, `invariant`, `contract`, `lint`, `format`, `schema`, `migration` |

   **Matching rules**:
   - Matching is **case-insensitive**
   - A convention matches a domain if **any** keyword from that domain's list appears in the convention text
   - A convention that matches **no** domain keyword list is included in **all** personas (shared convention)
   - A convention that matches **multiple** domain keyword lists is included in **each** matching persona

   After filtering, you have three sets:
   - `correctness_conventions`, `correctness_patterns`
   - `security_conventions`, `security_patterns`
   - `performance_conventions`, `performance_patterns`

## Step 4: Spawn Reviewer Remoras (Panel)

Load all three personas from the resolved `personas` context (or repo overrides if present):

1. `correctness.md`
2. `security.md`
3. `performance.md`

Each reviewer receives the **filtered** context for its domain — not the full unfiltered set. The reviewer prompt includes a `## Recurring Patterns` section between the context block and the diff. This section is **only included if filtered patterns exist** for that persona (i.e., 2+ reviews existed and patterns matching that domain were found).

Branch on `debate_available` (set in Step 2):

### Path A: Multi-Model Debate (`debate_available = true`)

When the `agent` CLI is available, each persona runs a **paired debate** — two models review the same diff through the same persona, and their findings are merged into Consensus/Disputes/Blindspots.

For each persona, construct:
- **system_prompt**: persona definition + filtered conventions + filtered recurring patterns (if any)
- **user_prompt**: the diff

Spawn **3 paired debate calls concurrently** via `collect()`. Each debate call is a single remora that internally dispatches both models and merges their output:

```
// All 3 debate pairs spawned concurrently via adapter collect()
// Each remora internally runs dispatch.sh twice + parse.py --merge
collect([
  {
    task: "Run a paired debate for the correctness persona.

    WORKSPACE=$(git rev-parse --show-toplevel)
    DISPATCH=~/.xavier/deps/multi-model-dispatch/dispatch.sh
    PARSE=~/.xavier/deps/multi-model-dispatch/parse.py

    SYSTEM_PROMPT={correctness.md + correctness_conventions + correctness_patterns, or omit patterns section}
    DIFF={diff}

    # 1. Dispatch to both models
    bash $DISPATCH gpt-5.4-xhigh $WORKSPACE /tmp/xavier-correctness-gpt.json \"$SYSTEM_PROMPT\" \"$DIFF\"
    bash $DISPATCH gemini-3.1-pro $WORKSPACE /tmp/xavier-correctness-gemini.json \"$SYSTEM_PROMPT\" \"$DIFF\"

    # 2. Merge into debate format
    python3 $PARSE --merge /tmp/xavier-correctness-gpt.json /tmp/xavier-correctness-gemini.json

    Return the merged Consensus/Disputes/Blindspots output.",
    name: "xavier correctness debate"
  },
  {
    task: "Run a paired debate for the security persona.

    WORKSPACE=$(git rev-parse --show-toplevel)
    DISPATCH=~/.xavier/deps/multi-model-dispatch/dispatch.sh
    PARSE=~/.xavier/deps/multi-model-dispatch/parse.py

    SYSTEM_PROMPT={security.md + security_conventions + security_patterns, or omit patterns section}
    DIFF={diff}

    # 1. Dispatch to both models
    bash $DISPATCH gpt-5.4-xhigh $WORKSPACE /tmp/xavier-security-gpt.json \"$SYSTEM_PROMPT\" \"$DIFF\"
    bash $DISPATCH gemini-3.1-pro $WORKSPACE /tmp/xavier-security-gemini.json \"$SYSTEM_PROMPT\" \"$DIFF\"

    # 2. Merge into debate format
    python3 $PARSE --merge /tmp/xavier-security-gpt.json /tmp/xavier-security-gemini.json

    Return the merged Consensus/Disputes/Blindspots output.",
    name: "xavier security debate"
  },
  {
    task: "Run a paired debate for the performance persona.

    WORKSPACE=$(git rev-parse --show-toplevel)
    DISPATCH=~/.xavier/deps/multi-model-dispatch/dispatch.sh
    PARSE=~/.xavier/deps/multi-model-dispatch/parse.py

    SYSTEM_PROMPT={performance.md + performance_conventions + performance_patterns, or omit patterns section}
    DIFF={diff}

    # 1. Dispatch to both models
    bash $DISPATCH gpt-5.4-xhigh $WORKSPACE /tmp/xavier-performance-gpt.json \"$SYSTEM_PROMPT\" \"$DIFF\"
    bash $DISPATCH gemini-3.1-pro $WORKSPACE /tmp/xavier-performance-gemini.json \"$SYSTEM_PROMPT\" \"$DIFF\"

    # 2. Merge into debate format
    python3 $PARSE --merge /tmp/xavier-performance-gpt.json /tmp/xavier-performance-gemini.json

    Return the merged Consensus/Disputes/Blindspots output.",
    name: "xavier performance debate"
  }
])
```

Each remora runs `dispatch.sh` twice (once per model) sequentially within itself, then merges the two outputs with `parse.py --merge`. The three remoras run concurrently with each other. The output of each remora is structured Markdown with `## Consensus`, `## Disputes`, and `## Blindspots` sections.

### Path B: Claude-Only Fallback (`debate_available = false`)

When the `agent` CLI is not available, fall back to the standard three-persona flow. No error, no warning, no mention of debate.

Spawn **3 reviewer agents concurrently** via the runtime adapter. All three must be spawned in a **single message** with parallel tool calls using `run_in_background: true`.

```
// All 3 spawned concurrently via adapter collect()
// Each receives ONLY the conventions and patterns for its domain
collect([
  {
    task: "You are a code reviewer...\n## Persona\n{correctness.md}\n## Context\n{correctness_conventions}\n## Recurring Patterns\n{correctness_patterns, or omit section}\n## Diff\n{diff}",
    name: "xavier correctness"
  },
  {
    task: "You are a code reviewer...\n## Persona\n{security.md}\n## Context\n{security_conventions}\n## Recurring Patterns\n{security_patterns, or omit section}\n## Diff\n{diff}",
    name: "xavier security"
  },
  {
    task: "You are a code reviewer...\n## Persona\n{performance.md}\n## Context\n{performance_conventions}\n## Recurring Patterns\n{performance_patterns, or omit section}\n## Diff\n{diff}",
    name: "xavier performance"
  }
])
```

Each reviewer receives the same diff but **domain-filtered** conventions and patterns, and reviews through the lens of their persona only. Reviewers should pay extra attention to recurring patterns — these represent issues that keep coming back.

## Step 5: Pilot Fish (Incremental Aggregation)

The pilot fish aggregates findings as each reviewer completes. It does NOT wait for all reviewers before starting.

**As each reviewer completes**, update the user with progress:
- "Reviewer 1/3 complete (correctness)..."
- "Reviewer 2/3 complete (security)..."
- "Reviewer 3/3 complete (performance)..."

The pilot fish handles two different input formats depending on which path was taken in Step 4:

### Format Detection

When a reviewer result arrives, check whether the output contains `## Consensus`, `## Disputes`, and `## Blindspots` headings. If all three are present, this is **debate format** (Path A). Otherwise, it is **raw findings format** (Path B fallback).

### Processing Debate Format (Path A output)

Each persona's debate output contains three sections. The pilot fish processes them as follows:

- **Consensus findings**: These carry high confidence (two models agree). Include them directly, tagged with the persona's category (e.g., `[correctness]`). Use the higher of the two severity levels if they differ.
- **Dispute findings**: These require human judgment. Include them with a `[disputed]` marker alongside the category tag. Present both sides concisely.
- **Blindspot findings**: These are coverage gaps found by only one model. Include them with a `[blindspot]` marker. They are still valid findings but carry lower confidence than consensus.

When applying vault interaction rules (from the debate protocol): if recurring patterns from the vault corroborate a Consensus finding, mark it as "confirmed." If recurring patterns contradict a Consensus finding, reclassify it as a Dispute. The pilot fish never creates new findings — it only reclassifies based on vault evidence.

### Processing Raw Findings Format (Path B output)

Each persona returns raw findings directly. Process them as in the standard flow — no consensus/dispute/blindspot classification applies.

### Synthesis (both paths)

**After all reviewers have reported**, synthesize:

1. **Categorize** all findings by type: correctness, security, performance
2. **Deduplicate**: if two reviewers flag the same line/issue, merge into a single finding and note which reviewers flagged it
3. **Rank by severity**: critical > high > medium > low. For debate-path output, rank within tiers: consensus findings above blindspot findings at the same severity level, disputed findings shown separately
4. **Determine final verdict**: the most severe individual verdict wins
   - If ANY reviewer says **rethink** -> final verdict is **rethink**
   - If ANY reviewer says **request changes** (and none say rethink) -> **request changes**
   - If ALL reviewers say **approve** -> **approve**
   - For debate-path output: a persona's verdict is derived from its most severe consensus finding. Disputes and blindspots alone do not escalate the verdict above **request changes**

## Step 6: Deliver Verdict

Present the synthesized review to the user:

1. Show the final verdict: **approve**, **request changes**, or **rethink**
2. Show per-reviewer verdicts: `correctness: approve | security: request changes | performance: approve`
3. List findings grouped by severity (critical > high > medium > low), with category tags
4. Show the total finding count and breakdown by category
5. Highlight any findings flagged by multiple reviewers (high confidence)

## Step 7: Write Review Note

Write a review note to `~/.xavier/knowledge/reviews/` with the following format:

**Filename**: `{repo-name}_{YYYY-MM-DD}_{content-hash}.md` where `content-hash` is the first 7 chars of `echo "{diff}" | shasum | cut -c1-7` (a hash of the diff content). This prevents filename collisions when running multiple reviews on the same commit.

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
