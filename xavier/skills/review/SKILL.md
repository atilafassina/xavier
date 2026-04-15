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
3. **Check debate availability**: Run both checks — the `agent` CLI and the dispatch scripts must both exist:
   ```bash
   command -v agent >/dev/null 2>&1 && test -x "${XAVIER_HOME:-$HOME/.xavier}/deps/multi-model-dispatch/dispatch.sh" && test -x "${XAVIER_HOME:-$HOME/.xavier}/deps/multi-model-dispatch/parse.sh"
   ```
   If the combined check exits 0, set `debate_available = true`. Otherwise set `debate_available = false`. **SILENT CHECK — NEVER print, emit, or mention the result of this check to the user. Do not say "falling back", "dispatch not installed", "debate not available", or anything about multi-model support. The user must not know this check happened.**

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
// Each remora internally runs dispatch.sh twice + parse.sh merge
collect([
  {
    task: "Run a paired debate for the correctness persona.

    WORKSPACE=$(git rev-parse --show-toplevel)
    DISPATCH=~/.xavier/deps/multi-model-dispatch/dispatch.sh
    PARSE=~/.xavier/deps/multi-model-dispatch/parse.sh
    TMPDIR=$(mktemp -d)

    SYSTEM_PROMPT={correctness.md + correctness_conventions + correctness_patterns, or omit patterns section}
    DIFF={diff}

    # 1. Dispatch to both models (mktemp paths prevent symlink attacks)
    bash $DISPATCH gpt-5.4-xhigh $WORKSPACE $TMPDIR/gpt.json \"$SYSTEM_PROMPT\" \"$DIFF\"
    bash $DISPATCH gemini-3.1-pro $WORKSPACE $TMPDIR/gemini.json \"$SYSTEM_PROMPT\" \"$DIFF\"

    # 2. Merge into debate format
    bash $PARSE merge $TMPDIR/gpt.json $TMPDIR/gemini.json GPT Gemini

    Return the merged Consensus/Disputes/Blindspots output.",
    name: "xavier correctness debate"
  },
  {
    task: "Run a paired debate for the security persona.

    WORKSPACE=$(git rev-parse --show-toplevel)
    DISPATCH=~/.xavier/deps/multi-model-dispatch/dispatch.sh
    PARSE=~/.xavier/deps/multi-model-dispatch/parse.sh
    TMPDIR=$(mktemp -d)

    SYSTEM_PROMPT={security.md + security_conventions + security_patterns, or omit patterns section}
    DIFF={diff}

    # 1. Dispatch to both models (mktemp paths prevent symlink attacks)
    bash $DISPATCH gpt-5.4-xhigh $WORKSPACE $TMPDIR/gpt.json \"$SYSTEM_PROMPT\" \"$DIFF\"
    bash $DISPATCH gemini-3.1-pro $WORKSPACE $TMPDIR/gemini.json \"$SYSTEM_PROMPT\" \"$DIFF\"

    # 2. Merge into debate format
    bash $PARSE merge $TMPDIR/gpt.json $TMPDIR/gemini.json GPT Gemini

    Return the merged Consensus/Disputes/Blindspots output.",
    name: "xavier security debate"
  },
  {
    task: "Run a paired debate for the performance persona.

    WORKSPACE=$(git rev-parse --show-toplevel)
    DISPATCH=~/.xavier/deps/multi-model-dispatch/dispatch.sh
    PARSE=~/.xavier/deps/multi-model-dispatch/parse.sh
    TMPDIR=$(mktemp -d)

    SYSTEM_PROMPT={performance.md + performance_conventions + performance_patterns, or omit patterns section}
    DIFF={diff}

    # 1. Dispatch to both models (mktemp paths prevent symlink attacks)
    bash $DISPATCH gpt-5.4-xhigh $WORKSPACE $TMPDIR/gpt.json \"$SYSTEM_PROMPT\" \"$DIFF\"
    bash $DISPATCH gemini-3.1-pro $WORKSPACE $TMPDIR/gemini.json \"$SYSTEM_PROMPT\" \"$DIFF\"

    # 2. Merge into debate format
    bash $PARSE merge $TMPDIR/gpt.json $TMPDIR/gemini.json GPT Gemini

    Return the merged Consensus/Disputes/Blindspots output.",
    name: "xavier performance debate"
  }
])
```

Each remora runs `dispatch.sh` twice (once per model) sequentially within itself, then merges the two outputs with `parse.sh merge`. The three remoras run concurrently with each other. The output of each remora is structured Markdown with `## Consensus`, `## Disputes`, and `## Blindspots` sections.

### Path B: Standard Three-Persona Flow (`debate_available = false`)

When `debate_available` is false, run the standard three-persona review. **Do not mention debate, multi-model, fallback, dispatch, or the `agent` CLI to the user — proceed as if this is the only review mode.**

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

### Vault Overlay (Debate Path Only)

After classifying all findings from the debate output, overlay vault knowledge by matching each consensus finding against the recurring patterns loaded in Step 3. The pilot fish never creates new findings — it only reclassifies based on vault evidence.

**Corroboration rule** — a vault recurring pattern **corroborates** a consensus finding when BOTH conditions are met:
1. The pattern's `category` field matches the finding's persona category (e.g., both are `correctness`)
2. The pattern's one-line description refers to the same issue type or code area as the finding (e.g., the pattern says "missing null check in API handlers" and the finding flags a missing null check in an API handler)

When corroborated: upgrade the consensus finding to **confirmed**. Tag it `[confirmed]` in addition to its category tag. Confirmed findings appear in the "High-Confidence Findings" section of the structured brief.

**Contradiction rule** — a vault recurring pattern **contradicts** a consensus finding when EITHER condition is met:
1. The pattern explicitly documents that the flagged code is **intentional** (e.g., "intentional any-type usage in serialization layer")
2. The pattern records that the same finding was **previously accepted** by the team in a past review (e.g., "accepted: raw SQL in migration scripts per team convention")

When contradicted: reclassify the consensus finding as a **dispute**. Move it from Consensus to Disputes, and note the vault evidence as the dissenting position. The pilot fish adds the vault context as a third perspective alongside the two model opinions.

**No match**: if no recurring pattern matches the finding's category and issue type, the finding keeps its original classification unchanged.

### Processing Raw Findings Format (Path B output)

Each persona returns raw findings directly. Process them as in the standard flow — no consensus/dispute/blindspot classification applies. No vault overlay is performed on raw findings.

### Synthesis (both paths)

**After all reviewers have reported**, synthesize:

1. **Categorize** all findings by type: correctness, security, performance
2. **Deduplicate**: if two reviewers flag the same line/issue, merge into a single finding and note which reviewers flagged it
3. **Rank by severity**: critical > high > medium > low. For debate-path output, rank within tiers: confirmed findings above consensus findings above blindspot findings at the same severity level, disputed findings shown separately
4. **Determine final verdict**: the most severe individual verdict wins
   - If ANY reviewer says **rethink** -> final verdict is **rethink**
   - If ANY reviewer says **request changes** (and none say rethink) -> **request changes**
   - If ALL reviewers say **approve** -> **approve**
   - For debate-path output: a persona's verdict is derived from its most severe consensus or confirmed finding. Disputes and blindspots alone do not escalate the verdict above **request changes**

## Step 6: Deliver Verdict

Branch on `debate_available` to determine the output format.

### Path A: Structured Brief (debate path)

When `debate_available` was true (debate path was taken in Step 4), present the synthesized review as a **structured brief**:

```
## Verdict: [approve | request changes | rethink]

correctness: {verdict} | security: {verdict} | performance: {verdict}

## TL;DR

2-3 sentences summarizing the state of this diff. What is the overall
quality? What is the single most important thing the author should know?

## High-Confidence Findings

Items where both models agreed (consensus) AND vault patterns confirm.
These are near-certain issues. Minimal explanation needed.

For each finding:
- **[severity]** **[category]** **[confirmed]** Brief description
  - File: `path/to/file.ext`, line {N}
  - Why: one-sentence explanation

If no findings were confirmed by vault patterns, show consensus findings
here instead (without the [confirmed] tag).

## Disputes Requiring Your Call

Items where models disagreed, or where vault evidence contradicted a
consensus finding. Use the matching format:

When GPT and Gemini disagreed:
- **[severity]** **[category]** **[disputed]** Brief description
  - GPT argued: {one-sentence summary of GPT's position}
  - Gemini argued: {one-sentence summary of Gemini's position}
  - Vault context: {what recurring patterns or team conventions say, or "no prior context" if none}
  - Pilot fish recommendation: {the pilot fish's suggested resolution based on evidence weight}

When vault evidence contradicted a consensus finding:
- **[severity]** **[category]** **[disputed]** Brief description
  - Models agreed: {one-sentence summary of the shared GPT/Gemini position}
  - Vault argued: {one-sentence summary of the vault's dissenting evidence or convention}
  - Vault context: {what recurring patterns or team conventions say}
  - Pilot fish recommendation: {the pilot fish's suggested resolution based on evidence weight}

## Blindspots

Items only one model caught. Flagged for awareness, not necessarily
actionable. For each:
- **[severity]** **[category]** **[blindspot]** Brief description
  - Caught by: {GPT | Gemini}
  - File: `path/to/file.ext`, line {N}

## Recurring Pattern Matches

Known issues from past reviews that showed up again in this diff.
For each matched pattern:
- Pattern: "{one-line description from vault}"
  - Recurrence count: {N} times across past reviews
  - Matched finding(s): {which finding(s) this pattern corroborated or contradicted}
  - Status: {confirmed consensus | reclassified as dispute | informational}

If no recurring patterns matched, show: "No recurring patterns matched this diff."
```

**Decision gate**: After presenting the structured brief, pause and prompt the user:

> Review the brief above. The review note will be written after you respond.

Do NOT proceed to Step 7 until the user acknowledges. No auto-fix, no auto-push — the user decides what to do with the findings. Wait for any response from the user before continuing.

### Path B: Standard Verdict (`debate_available = false`)

When `debate_available` was false, present the synthesized review in the standard format:

1. Show the final verdict: **approve**, **request changes**, or **rethink**
2. Show per-reviewer verdicts: `correctness: approve | security: request changes | performance: approve`
3. List findings grouped by severity (critical > high > medium > low), with category tags
4. Show the total finding count and breakdown by category
5. Highlight any findings flagged by multiple reviewers (high confidence)

There is no decision gate — proceed directly to Step 7.

## Step 7: Write Review Note

Write a review note to `~/.xavier/knowledge/reviews/` with the following format:

**Filename**: `{repo-name}_{YYYY-MM-DD}_{content-hash}.md` where `content-hash` is the first 7 chars of `echo "{diff}" | shasum | cut -c1-7` (a hash of the diff content). This prevents filename collisions when running multiple reviews on the same commit.

Branch on `debate_available` to determine the review note format.

### Path A: Debate Review Note (`debate_available = true`)

When the debate path was taken, write the review note with extended frontmatter and a Decisions section. Parse the user's response to the decision gate (from Step 6) to populate the Decisions table.

```markdown
---
repo: {current repo name}
module: {most-changed directory in the diff}
type: review
verdict: {approve | request-changes | rethink}
finding-categories: [{list of categories found, e.g. correctness, security, performance}]
recurring: [{findings that appeared in past reviews of this repo}]
models: [gpt-5.4-xhigh, gemini-3.1-pro]
debate-mode: true
decisions-recorded: true
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

## Decisions

| Finding | Source | User Decision | Rationale |
|---------|--------|---------------|-----------|
| {short finding description} | {source — see format rules below} | {Accepted / Rejected / Deferred / Acknowledged} | {user's rationale, or "—" if none given} |

## Context

- **Diff scope**: {number of files changed, insertions, deletions}
- **Reviewers**: correctness, security, performance
- **Models**: gpt-5.4-xhigh, gemini-3.1-pro
```

**Rules for populating the Decisions table:**

- Every finding from the structured brief gets a row
- High-confidence/confirmed findings: default to "Acknowledged" unless the user explicitly disagrees
- Disputed findings: record which side the user chose (or "Deferred" if they did not decide)
- Blindspot findings: "Acknowledged" by default, "Rejected" if user dismisses
- If the user does not address a specific finding, use "Acknowledged" for consensus/confirmed, "Deferred" for disputes

**Decision log parseability constraints** (required for Step 8 recurring-pattern extraction):

- The **Source** column must use one of these exact formats:
  - `Consensus` — both models agreed
  - `Consensus (confirmed)` — both models agreed and vault patterns corroborated
  - `Dispute (GPT: {position}, Gemini: {position})` — models disagreed, with each model's position summarized in a few words
  - `Blindspot ({model})` — only one model caught the finding, naming which model
- The **User Decision** column must use exactly one of: `Accepted`, `Rejected`, `Deferred`, `Acknowledged`
- These constraints enable automated parsing in Step 8

### Path B: Standard Review Note (`debate_available = false`)

When `debate_available` was false, write the review note with standard frontmatter. No Decisions section is added.

```markdown
---
repo: {current repo name}
module: {most-changed directory in the diff}
type: review
verdict: {approve | request-changes | rethink}
finding-categories: [{list of categories found, e.g. correctness, security, performance}]
recurring: [{findings that appeared in past reviews of this repo}]
debate-mode: false
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

## Step 8: Recurring Pattern Feedback

This step only runs when `debate_available` was true. For fallback-path reviews, skip this step entirely.

After writing the review note in Step 7, run the recurring-pattern feedback loop. This surfaces cases where the user has historically overridden a model's findings, so the pilot fish can account for those preferences in future reviews.

### Detection Logic

1. Scan `~/.xavier/knowledge/reviews/` for all review notes matching the current repo (where frontmatter `repo` matches and `debate-mode: true`).
2. Parse the `## Decisions` table from each past review note. For each row, extract: **Finding** (description), **Source** (which model or consensus), **User Decision** (Accepted/Rejected/Deferred/Acknowledged), and the finding's category (inferred from the finding's tag in the `## Findings` section).
3. Group overrides by: `(category, source model, finding description similarity)`. An override is any row where User Decision is `Rejected` or `Deferred`.
4. If the same type of finding (same category + similar description) from the same model has been overridden in **3 or more** past reviews, flag it as a recurring override pattern.

### Surfacing Patterns

When a recurring override pattern is detected:

1. Record it in the current review note's frontmatter `recurring` list (it already appears there if the recurring-patterns context flagged it in Step 3).
2. For **future reviews** of the same repo, the recurring-patterns context (loaded in Step 3) will include this pattern. The pilot fish should present it in the structured brief as:

   > "You historically disagree with {model} on {category} findings like this — consider this when weighing their argument."

3. This annotation appears in the `## Disputes Requiring Your Call` section of the structured brief, alongside the pilot fish recommendation, when a finding matches a known override pattern.

### Heuristics and Limitations

- **Similarity matching is soft**: The pilot fish uses judgment to determine whether two finding descriptions refer to the same type of issue. Exact string matching is not required — semantic similarity (same category, same code pattern, same kind of concern) is sufficient.
- **Only debate-path reviews contribute**: Fallback-path reviews do not have a `## Decisions` table and are excluded from override counting.
- **The threshold is 3**: Fewer than 3 overrides on the same pattern do not trigger the feedback annotation. This prevents noise from one-off disagreements.
- **The feedback is advisory, not prescriptive**: The pilot fish presents the pattern as context, not as a directive to ignore the model's finding. The user still makes the final call.
