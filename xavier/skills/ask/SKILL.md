---
name: ask
description: Answer focused questions about a repo using captured team knowledge; spawn research only when the vault is thin.
requires: [shark, adapter, repo-conventions:optional, team-conventions:optional, recurring-patterns:optional, research-index:optional, investigations-index:optional, qa-index:optional]
---

# Ask

`/xavier ask "<question>"`

Answer a focused question about the current repository by synthesizing captured team knowledge from the Xavier vault. Reads decisions, architecture, dependencies, team conventions, recurring review patterns, and (on relevance match) prior research and investigation notes. Presents a TL;DR + Evidence + Sources answer inline.

When the vault is thin on the topic, the skill prompts before spawning research remoras (1 narrow / 3 design / 5 exploratory) and auto-saves the resulting answer to `<vault>/knowledge/qa/` so future asks can reuse it. Bare invocation (`/xavier ask` with no question) prompts once for the question; `--repo <name>` overrides the cwd-derived repo scope.

## Step 1: Detect-and-Defer

Follow the detect-and-defer protocol from the Shark reference:

```bash
echo "$SHARK_TASK_HASH"
```

- **If set** (non-empty): act as a simple Q&A executor — read the vault inline, synthesize an answer, print it. No interactive prompts.
- **If unset** (empty): proceed as top-level orchestrator with the full flow below.

## Step 2: Parse Input

Inputs come in two shapes: an optional `--repo <name>` override and an optional positional question. Both are independent — either, both, or neither may be present. Process flags first, then the question.

### 2.1 — Extract `--repo <name>` flag (if present)

Scan the args (in any order) for a `--repo <name>` flag. The flag can appear before or after the question — both `/xavier ask --repo appkit "<q>"` and `/xavier ask "<q>" --repo appkit` are valid.

1. If `--repo` appears with no value following it, abort with: `Error: --repo flag requires a value. Usage: /xavier ask --repo <name> "<question>".` Then stop.
2. If `--repo` appears more than once, abort with: `Error: --repo flag specified more than once. Usage: /xavier ask --repo <name> "<question>".` Then stop.
3. Validate the captured `<name>` against the `knowledge/repos` segment grammar from the router's wikilink validation rules:

   ```
   ^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$
   ```

   Reject any value that contains `/`, `\`, `..`, leading `.`, whitespace, control characters, an absolute path, or any character outside the grammar. On invalid name, abort BEFORE any filesystem read with: `Error: --repo value "<name>" is not a valid repo segment. Expected pattern: ^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$ (no slashes, no '..', no leading '.', no whitespace).` Then stop. Quote the bad input verbatim in the error so the user can see what was rejected.
4. If valid, store the value as the `REPO_NAME` override. Step 3's cwd-derivation logic is skipped — use the override directly.

Remove the `--repo <name>` tokens from the arg list before continuing to 2.2.

### 2.2 — Extract positional question

After flag extraction, treat the first remaining positional argument as the question string. Trim surrounding whitespace and matching surrounding quotes (a single matched pair of `"` or `'`).

### 2.3 — Bare invocation (no positional question)

If no positional question remains after 2.1 and 2.2:

- **Detect-and-defer**: if Step 1 found `SHARK_TASK_HASH` set (non-empty), print this short error and exit cleanly: `Error: ask remora was invoked without a question; cannot continue.` Do NOT call `AskUserQuestion` — remoras cannot drive interactive prompts.
- **Interactive (no `SHARK_TASK_HASH`)**: fire a single `AskUserQuestion` with the body `What's your question?` (free-text answer expected). After the user supplies the question, trim whitespace and matching surrounding quotes the same way as 2.2 and treat the result as the question string. Single-turn — no follow-up loop. If the user returns an empty string, abort with: `Error: no question provided. Aborting.` Then stop.

Proceed to Step 3 once a non-empty question string is in hand.

## Step 3: Resolve Repo Scope

Resolve the current repository name. There are two derivation paths — flag override and cwd auto-detect — but the existence check that follows applies to both.

**3a. Derive `REPO_NAME`:**

- **If Step 2.1 set a `REPO_NAME` override via `--repo`**: use that value directly. Skip the cwd-derivation logic below.
- **Otherwise (no `--repo` flag)**, auto-detect from cwd:
  1. Try `git remote get-url origin 2>/dev/null`. If it succeeds, take the trailing path segment of the URL and strip a trailing `.git` suffix. Use that as `REPO_NAME`.
  2. If the git remote lookup fails or returns nothing, fall back to `basename "$(pwd)"`.

**3b. Existence check (applies to both derivation paths):**

3. Check whether `<vault>/knowledge/repos/<REPO_NAME>/` exists as a directory. If it does, proceed to Step 4 with the vault-scoped flow.
4. **Empty-vault edge case**: if `<vault>/knowledge/repos/<REPO_NAME>/` does NOT exist (no captured knowledge for this repo), skip Steps 4–6 entirely. The vault read and team-convention resolution have nothing to surface and the sufficiency assessment would always trip the floor rule. Jump straight to Step 8 (Research Fallback) with the following adjustments:
   - The `AskUserQuestion` prompt body MUST append this tip on a new line:

     > 💡 Run `/xavier learn` to cache repo knowledge — future asks will be much faster.

   - The vault-summary block passed to each remora (Step 8) is the literal string `"(no captured knowledge for repo {REPO_NAME})"`.
   - On decline, print a one-liner: "No captured knowledge for `{REPO_NAME}` and research declined — nothing to surface. Run `/xavier learn` to cache repo knowledge for next time." Then exit.
   - On accept, the rest of Steps 8–9 (adaptive remoras, synthesis, auto-save) run unchanged.

   Detect-and-defer remains in effect: when invoked inside an outer Shark loop (Step 1 found `SHARK_TASK_HASH` set), this branch must NOT fire the `AskUserQuestion` prompt. Instead, return a short inline message — "No vault context for `{REPO_NAME}`; remora cannot answer without spawning sub-agents." — and exit cleanly.

## Step 4: Resolve Team Conventions via Wikilinks

Team-convention resolution is wikilink-driven — there is no global team scan and no user prompt.

1. Read `<vault>/knowledge/repos/<REPO_NAME>/decisions.md` and `<vault>/knowledge/repos/<REPO_NAME>/architecture.md` if they exist.
2. For each file, parse the `related:` frontmatter list. Extract every wikilink whose target matches one of:
   - `[[knowledge/teams/<team>/conventions]]`
   - `[[knowledge/teams/<team>]]` (directory-style)
3. For each matched wikilink, load the corresponding team-convention file:
   - `[[knowledge/teams/<team>/conventions]]` → `<vault>/knowledge/teams/<team>/conventions.md`
   - `[[knowledge/teams/<team>]]` → `<vault>/knowledge/teams/<team>/conventions.md` (the canonical file in that team directory)
4. If `decisions.md` and `architecture.md` carry no team wikilinks, skip silently — do not warn, do not prompt, do not scan other teams.

## Step 5: Vault Read

Load context with a two-tier strategy: full read for repo-scoped notes, index-only read for broader corpora with full-read on relevance match.

**Full read (always):**

- Every `.md` file under `<vault>/knowledge/repos/<REPO_NAME>/` — including `decisions.md`, `architecture.md`, `dependencies.md`, and any per-workspace files (e.g., `<vault>/knowledge/repos/<REPO_NAME>/<package>/dependencies.md`).
- The team-convention files matched in Step 4.
- Recurring patterns for the current repo — already resolved by the router via the `recurring-patterns` requires key. Use the resolved context directly; do not re-read `knowledge/reviews/`.

**Index-only read + relevance match (full read on hit):**

- The `research-index`, `investigations-index`, and `qa-index` requires keys are pre-resolved by the router. They surface titles and frontmatter from `<vault>/research/`, `<vault>/investigations/`, and `<vault>/knowledge/qa/` respectively — no body content.
- Extract salient nouns from the question (lowercase content words, stop-words filtered).
- For each index entry, check whether any salient noun overlaps the entry's title, `topic` / `symptom` / `question` frontmatter, `tags`, or filename slug. The `question` field is the primary identifier for prior Q&A notes — match against it the same way `topic` is matched for research and `symptom` is matched for investigations.
- For each entry that matches, full-read that note's body. Cite it in Step 7 (or Step 9, on the research-fallback path) like any other source — prior Q&A notes are cited via `[[knowledge/qa/<filename>]]` wikilinks.
- Entries with no overlap stay index-only — their bodies are never loaded. Token cost stays bounded.

## Step 6: Assess Vault Sufficiency

Decide whether the loaded context can produce an answer with concrete citations. The check has two layers — a deterministic floor rule and a soft model judgment layer above it. Either layer firing the "thin" verdict routes the flow to Step 8; otherwise route to Step 7.

**Floor rule (deterministic, always applied first):**

1. Tokenize the question text: lowercase, strip punctuation and quotes, split on whitespace.
2. Filter stop-words. Use this short list:

   ```
   a, an, the, is, are, was, were, do, does, did,
   what, why, how, when, where, who, which,
   of, in, on, at, to, for, and, or, but, with, from, by,
   our, my, this, that, these, those
   ```

3. The remaining tokens are the **salient nouns** for the question.
4. For each salient noun, check whether it appears anywhere in the body text of any note loaded in Step 5 (case-insensitive substring match — bodies, not just titles or frontmatter).
5. **If none of the salient nouns appear in any loaded note's body, the vault is thin.** Route to Step 8.

**Model judgment (soft layer, only if floor rule did not trip):**

After the floor rule passes, attempt to draft the synthesis internally. Before printing it, ask: "can the answer be produced from these notes with at least one concrete `[[wikilink]]` citation in Evidence?"

- **Yes**: vault is sufficient. Proceed to Step 7.
- **No** (the draft would be hedged, generic, or have no real citations even though some salient noun matched): treat the vault as thin. Route to Step 8.

The floor rule is the deterministic gate that catches the obvious thin-vault case. Model judgment is the soft layer that catches the case where salient nouns appear in passing but not in a way that supports an answer.

**Detect-and-defer note**: when invoked inside an outer Shark loop (Step 1 found `SHARK_TASK_HASH` set), the model-judgment layer still routes a thin verdict — but Step 8 must NOT fire the `AskUserQuestion` prompt and MUST NOT spawn remoras. Instead, fall straight to the Step 7 partial-answer output with the gap-note footer (same as the user-declined branch of Step 8). The skill exits without prompts or saves.

## Step 7: Synthesize and Present (Vault-Sufficient)

Synthesize the answer in TL;DR + Evidence + Sources format using the loaded context. Print inline. The vault-only save UX in Step 10 runs after this step for the sufficient branch only.

**Output template:**

```markdown
## TL;DR

<one-line direct answer to the question>

## Evidence

<two-to-five short paragraphs of supporting reasoning. Every claim that comes from a vault note carries an inline [[wikilink]] to the source note. For example: "The repo uses pnpm as its package manager — see [[knowledge/repos/<repo>/decisions]] for the rationale.">

## Sources

- [[knowledge/repos/<repo>/decisions]]
- [[knowledge/repos/<repo>/architecture]]
- [[knowledge/teams/<team>/conventions]]
- [[research/<related-note>]]
- [[investigations/<related-note>]]
```

**Rules:**

- Every `[[wikilink]]` in Evidence and Sources MUST resolve to a real file that was actually loaded in Step 5. Do not fabricate citations.
- TL;DR is a single sentence — direct answer first, no preamble.
- Evidence stays under five paragraphs. Restructure and connect findings across notes — do not copy-paste note content verbatim.
- Sources is a deduplicated wikilink list. Include every note cited inline in Evidence, plus any additional vault notes that informed the answer.

**Partial-answer branch (only on the thin-vault detect-and-defer path or the Step 8 user-declined path)**:

If the flow reached this step from a thin-vault verdict (detect-and-defer in effect, or user declined research in Step 8), still return the best partial answer the loaded context supports. Append a single italicized line at the end of the Evidence section — choose the variant matching how the thin verdict was reached:

  > *Vault context is thin for this question — captured knowledge did not cover all aspects of the asked topic. Re-run interactively (outside a shark loop) to spawn research remoras, or run `/xavier learn` to capture more repo knowledge first.*

  or, on user-declined:

  > *Vault context is thin for this question and research was declined — partial answer only.*

After printing the synthesis on the partial-answer branch, the skill is done. No save, no follow-up, no remora spawn. The vault-only save prompt in Step 10 is skipped for partial answers — only the fully sufficient synthesis path flows into Step 10.

## Step 8: Research Fallback (User-Confirmed)

This step runs only when Step 6 (or the Step 3 empty-vault branch) routes here AND Step 1 did NOT find `SHARK_TASK_HASH` set. Detect-and-defer takes precedence — inside an outer Shark loop the flow already fell through to Step 7's partial-answer branch.

### 8.1 — Prompt the user

Fire a single `AskUserQuestion` with the following body and two response options:

> Vault is thin on this topic. Spawn research remoras to fill gaps?
>
> *(If routed here from the empty-vault edge case in Step 3, append the `/xavier learn` tip from that step on a new line.)*

- **Option `yes`**: proceed to 8.2 (classify) and 8.3 (spawn).
- **Option `no`**: print the best partial answer the loaded vault supports using the Step 7 template, append the user-declined gap-note footer ("*Vault context is thin for this question and research was declined — partial answer only.*"), and exit. **Do not save.** **Do not spawn remoras.**

### 8.2 — Classify the question shape

Classify the question text by model judgment into exactly one of three shapes. The shape determines the remora count.

- **Narrow factual** (`N = 1`): the question asks for a single piece of locatable information — "where is X defined", "does this repo use Y", "what file does Z", "is there a config for W". A single targeted Explore grep + read suffices.
- **Design / why** (`N = 3`): the question asks for rationale, pattern, or convention — "why did we pick X", "how does our auth pattern work", "what's our convention for Z", "how is Y wired up here". Requires cross-axis triangulation (code + history + prior notes).
- **Exploratory / open-ended** (`N = 5`): the question is broad or comparative — "what are the options for X across teams", "what approaches has the team tried for Y". This is the upper bound; questions broader than this should run `/xavier research` instead. The Step 9 synthesis will append a hint at the end of the answer suggesting `/xavier research` for a fuller digest.

### 8.3 — Spawn remoras via `adapter.collect()`

All remoras spawn concurrently in a **single message** via the adapter's `collect()` with `run_in_background: true`. The adapter is the only spawn entry point — never invoke raw sub-agent constructors. The adapter injects `SHARK_TASK_HASH` into each remora's prompt preamble automatically; the prompt template still references it explicitly per skill convention.

**Remora axes by shape:**

For `N = 1` (narrow factual) — one remora:

| # | Axis label | Instructions | `subagent_type` |
|---|---|---|---|
| 1 | Repo grep | Locate the relevant code, config, or definition in `{cwd}` via Grep/Glob/Read. Report file paths, line ranges, and the snippet that answers the question. | `Explore` |

For `N = 3` (design / why) — three remoras, all spawned in one `collect()` call:

| # | Axis label | Instructions | `subagent_type` |
|---|---|---|---|
| 1 | Repo grep | Find code, configs, and inline documentation in `{cwd}` that touches the topic. Use Grep/Glob/Read. Report file paths and concrete snippets. | `Explore` |
| 2 | Git history | Run `git log --grep` and `git log -S` for terms derived from the question. Inspect commits and PR descriptions that introduced the pattern or made the decision. Report commit SHAs, dates, and one-line summaries. | `general-purpose` |
| 3 | Vault deep-scan | Full-read every `research/` and `investigations/` note flagged by Step 5's index relevance match (`{flagged_notes}` — passed in via the vault summary). Surface concrete claims and quote them. Cite each note via `[[wikilink]]`. | `Explore` |

For `N = 5` (exploratory / open-ended) — the three above plus two broader-scope remoras, all spawned in one `collect()` call:

| # | Axis label | Instructions | `subagent_type` |
|---|---|---|---|
| 1 | Repo grep | As above (3-case row 1). | `Explore` |
| 2 | Git history | As above (3-case row 2). | `general-purpose` |
| 3 | Vault deep-scan | As above (3-case row 3). | `Explore` |
| 4 | README + docs sweep | Read the repo's `README.md` and every `.md` file under `docs/` (if present). Surface high-level architectural or onboarding context relevant to the question. | `Explore` |
| 5 | Adjacent repos | Survey sibling repos in `<vault>/knowledge/repos/`. For each adjacent repo's `decisions.md` and `architecture.md` whose salient nouns overlap the question, surface the relevant claim with a `[[wikilink]]`. Do NOT cite repos that don't overlap. | `Explore` |

After Step 9's synthesis, the 5-case (and only the 5-case) appends a single line at the very end of the printed answer:

> *This looks like a broad topical question — consider `/xavier research` for a fuller digest.*

### 8.4 — Remora prompt template

Use this exact template per remora, substituting `{hash}`, `{repo}`, `{cwd}`, `{question}`, `{N}`, `{axis_label}`, `{axis_instructions}`, and `{vault_summary}`. The `{vault_summary}` block is a short bulleted recap of what Step 5 loaded (filenames and one-line gist per loaded note); for the empty-vault edge case from Step 3 it is the literal `"(no captured knowledge for repo {repo})"`. The `{flagged_notes}` placeholder used in the 3-case and 5-case Vault deep-scan axis is a bulleted list of wikilinks for the notes that Step 5's index relevance match flagged.

```
Export SHARK_TASK_HASH={hash} before starting work.

Answer the following question about repo "{repo}" (root: {cwd}):

{question}

You are one of {N} remoras working in parallel. Your specific axis:
{axis_label}: {axis_instructions}

Vault context already loaded (may be thin or empty):
{vault_summary}

Constraints:
- Return a concise factual answer under 400 words
- End with a ### Sources subsection listing every URL and file path you consulted
- Do NOT make recommendations — report what you find
- Do NOT spawn sub-agents
```

Once `collect()` returns with all remora results, advance to Step 9.

## Step 9: Synthesize and Persist (Research Fallback)

Combine the remora reports with the (possibly thin) vault context loaded in Step 5 and produce a single answer in the same TL;DR + Evidence + Sources structure used by Step 7. Then auto-save the answer to the vault.

### 9.1 — Synthesize

Same output shape as Step 7:

```markdown
## TL;DR

<one-line direct answer>

## Evidence

<two-to-five short paragraphs. Cite vault notes with inline [[wikilinks]]. Cite external URLs and file paths surfaced by remoras inline (URLs in parens, file paths in backticks). Connect findings across remora axes — do not copy-paste remora output.>

## Sources

- [[knowledge/repos/<repo>/decisions]]            # only the vault notes actually used
- [[research/<flagged-note>]]
- `path/to/file.ts:42-58` — what it shows         # remora-surfaced file paths
- [Title](https://example.com) — one-liner       # remora-surfaced external URLs
```

Rules:

- Every `[[wikilink]]` must resolve to a real loaded note (Step 5) or a real remora-cited file path.
- TL;DR is a single sentence.
- Evidence stays under five paragraphs.
- Sources is deduplicated. Merge URLs and file paths across all remoras.
- For the 5-case (exploratory) shape, append at the very end (after Sources):

  > *This looks like a broad topical question — consider `/xavier research` for a fuller digest.*

### 9.2 — Slug derivation

Derive the filename slug from the question text:

1. Lowercase the question.
2. Strip punctuation, quotes, and any non-`[a-z0-9 -]` characters (replace with space).
3. Tokenize on whitespace.
4. Filter stop-words using the same list as Step 6's floor rule:

   ```
   a, an, the, is, are, was, were, do, does, did,
   what, why, how, when, where, who, which,
   of, in, on, at, to, for, and, or, but, with, from, by,
   our, my, this, that, these, those
   ```

5. Take the first 5 remaining tokens (fewer if the question is shorter).
6. Join with `-`.

Example: question "Why did we pick pnpm over npm?" → tokens `["why","did","we","pick","pnpm","over","npm"]` → after stop-word filter `["we","pick","pnpm","over","npm"]` → first 5 → slug `we-pick-pnpm-over-npm`. (Note: `we`, `pick`, `pnpm`, `over`, `npm` are all retained because they are not in the stop-word list.)

### 9.3 — Filename and collision handling

Target path: `<vault>/knowledge/qa/{REPO_NAME}_{YYYY-MM-DD}_{slug}.md` where `{YYYY-MM-DD}` is today's date (the same value used in `created:` / `updated:` frontmatter — derive once and reuse).

Collision resolution: if the target path already exists, append a numeric suffix before the `.md` extension — `-2`, `-3`, … — incrementing until a free slot is found. No hash, no time-of-day suffix. The numeric suffix is deterministic and human-readable.

Examples:
- Free slot: `xavier_2026-05-20_we-pick-pnpm-over-npm.md`
- First collision: `xavier_2026-05-20_we-pick-pnpm-over-npm-2.md`
- Second collision: `xavier_2026-05-20_we-pick-pnpm-over-npm-3.md`

### 9.4 — Directory bootstrap

Ensure the target directory exists before writing:

```bash
mkdir -p "<vault>/knowledge/qa"
```

`mkdir -p` is idempotent — safe to run on every research-fallback save. No other scaffolding is required; the directory is created on first save.

### 9.5 — Write the note

Write the synthesized answer to the resolved target path with this Zettelkasten frontmatter:

```yaml
---
repo: {REPO_NAME}
type: qa
question: "{original question text, verbatim, with internal double-quotes escaped as \\\"}"
created: {YYYY-MM-DD}
updated: {YYYY-MM-DD}
tags:
  - qa
  - {optional question-derived tags}
related:
  - "[[knowledge/repos/{REPO_NAME}/decisions]]"
  - "[[<other vault notes cited inline in Evidence>]]"
sources:
  - "{external URL from remora, if any}"
  - "{file path from remora, if any}"
---
```

Then the body — the full TL;DR + Evidence + Sources synthesis from 9.1, unmodified.

**Frontmatter rules:**

- `repo` is always the auto-detected `REPO_NAME` from Step 3 (or the empty-vault repo name if routed from there).
- `type` is always the literal string `qa`.
- `question` carries the original question text verbatim. Escape internal double-quotes.
- `created` and `updated` are the same date on first save.
- `tags` always includes `qa`. Add 0–3 question-derived tags (kebab-case salient nouns) — optional, used for cross-cutting search.
- `related` lists `[[wikilink]]` entries for every vault note cited in Evidence. Omit the field entirely if no vault notes were cited (rare — only when the empty-vault edge case fired with no flagged research/investigation notes).
- `sources` lists external URLs and file paths surfaced by remoras, one per line. Omit the field if no external sources were cited.

### 9.6 — Announce the save

After writing, print one line below the synthesized answer:

> Saved to `knowledge/qa/{REPO_NAME}_{YYYY-MM-DD}_{slug}.md`.

(Use the post-collision filename if a suffix was applied.) The skill is now done. No follow-up, no further prompts.

## Step 10: Vault-Only Save Prompt (Sufficient Path)

This step runs only on the fully sufficient synthesis branch of Step 7 — Step 6 routed there, Step 7 produced an answer that was NOT the partial-answer branch (no thin-vault gap-note footer), and the research-fallback path of Steps 8–9 did not execute. Vault-only answers are asymmetric with research-fallback answers: research-fallback output is auto-saved (Step 9) because it carries net-new external info, while vault-only output is redundant with its source notes and the save is therefore opt-in with a default of No.

**Detect-and-defer**: when Step 1 found `SHARK_TASK_HASH` set, this step MUST NOT run. Skip the prompt entirely — no `AskUserQuestion`, no save. The remora exits cleanly after Step 7's print. This applies regardless of how the sufficient branch was reached.

### 10.1 — Prompt the user

Fire a single `AskUserQuestion` with the following body and two response options:

> Save this answer to vault? (yes/no)

- **Option `no`** (default): do nothing. The skill exits cleanly. No file is written.
- **Option `yes`**: proceed to 10.2 (save).

The default is No because vault-only synthesis is redundant with its source notes — saving would duplicate information already captured under `knowledge/repos/<repo>/`, `knowledge/teams/<team>/`, `research/`, or prior `knowledge/qa/` entries. The opt-in exists for the case where the user wants to capture the synthesis itself (e.g., the cross-note framing is novel and worth caching).

### 10.2 — Save (yes branch)

Reuse Step 9's slug derivation, filename collision handling, directory bootstrap, frontmatter schema, and announce-the-save logic exactly. The vault-only save path is identical to the research-fallback save path with one difference: there is no remora-surfaced external content, so the `sources` frontmatter field is omitted (no URLs or file paths from remoras to record) and the `related` list contains only the vault wikilinks cited in Evidence.

Specifically, follow these sub-steps from Step 9 in order, with no modification:

- **9.2 — Slug derivation**: derive the slug from the original question text using the stop-word filter and 5-token cap.
- **9.3 — Filename and collision handling**: target `<vault>/knowledge/qa/{REPO_NAME}_{YYYY-MM-DD}_{slug}.md`, append `-2`, `-3`, … on collision.
- **9.4 — Directory bootstrap**: `mkdir -p "<vault>/knowledge/qa"` before writing.
- **9.5 — Write the note**: emit the Zettelkasten frontmatter exactly as defined in 9.5 (with the field-omission note above — drop `sources` when empty), followed by the Step 7 synthesis body (TL;DR + Evidence + Sources) unmodified.
- **9.6 — Announce the save**: print the `Saved to knowledge/qa/{...}.md.` line below the already-printed answer.

Do not duplicate the schema or slug logic inline — point at Step 9. The single-source-of-truth keeps the two save paths from drifting.

After 9.6 prints (or the user declined in 10.1), the skill is done. No follow-up, no further prompts.
