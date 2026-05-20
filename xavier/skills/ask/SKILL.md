---
name: ask
description: Answer focused questions about a repo using captured team knowledge; spawn research only when the vault is thin.
requires: [shark, adapter, repo-conventions:optional, team-conventions:optional, recurring-patterns:optional, research-index:optional, investigations-index:optional]
---

# Ask

`/xavier ask "<question>"`

Answer a focused question about the current repository by synthesizing captured team knowledge from the Xavier vault. Reads decisions, architecture, dependencies, team conventions, recurring review patterns, and (on relevance match) prior research and investigation notes. Presents a TL;DR + Evidence + Sources answer inline.

This is Phase 1 of the skill: vault-only. Research-fallback, save UX, `--repo` flag, and bare-invocation prompt land in later phases.

## Step 1: Detect-and-Defer

Follow the detect-and-defer protocol from the Shark reference:

```bash
echo "$SHARK_TASK_HASH"
```

- **If set** (non-empty): act as a simple Q&A executor — read the vault inline, synthesize an answer, print it. No interactive prompts.
- **If unset** (empty): proceed as top-level orchestrator with the full flow below.

## Step 2: Parse Input

1. Treat the first positional argument as the question string. Trim surrounding whitespace and quotes.
2. If no positional argument is provided, error out with: "Error: `/xavier ask` requires a question. Usage: `/xavier ask \"<question>\"`. Bare invocation will be supported in a later phase." Then stop.

(The `--repo <name>` flag and bare-invocation prompt are deferred to Phase 4 — do not implement them here.)

## Step 3: Auto-Detect Repo from cwd

Resolve the current repository name:

1. Try `git remote get-url origin 2>/dev/null`. If it succeeds, take the trailing path segment of the URL and strip a trailing `.git` suffix. Use that as `REPO_NAME`.
2. If the git remote lookup fails or returns nothing, fall back to `basename "$(pwd)"`.
3. Check whether `<vault>/knowledge/repos/<REPO_NAME>/` exists as a directory. If it does, use that. If it does not, leave `REPO_NAME` set but mark the vault as empty for this repo — Step 5 will load whatever team / cross-repo context is available and Step 6 will flag the result as a partial answer. (The full empty-vault flow — including the `/xavier learn` tip and research-fallback prompt — is deferred to Phase 2.)

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

- The `research-index` and `investigations-index` requires keys are pre-resolved by the router. They surface titles and frontmatter from `<vault>/research/` and `<vault>/investigations/` respectively — no body content.
- Extract salient nouns from the question (lowercase content words, stop-words filtered).
- For each index entry, check whether any salient noun overlaps the entry's title, `topic`/`symptom` frontmatter, `tags`, or filename.
- For each entry that matches, full-read that note's body. Cite it in Step 6 like any other source.
- Entries with no overlap stay index-only — their bodies are never loaded. Token cost stays bounded.

## Step 6: Synthesize and Present

Synthesize the answer in TL;DR + Evidence + Sources format using the loaded context. Print inline. **Do not save** — vault-only save UX lands in Phase 3.

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
- If the vault context is thin — the loaded notes do not cover the question's salient nouns, or the model judges the answer would lack concrete citations — still return the best partial answer you can, then append a single italicized line at the end of the Evidence section:

  > *Vault context may be incomplete for this question — captured knowledge did not cover all aspects of the asked topic.*

  Do not prompt to spawn research remoras (that's Phase 2). Do not suggest a save (that's Phase 3). Just return what the vault surfaced and stop.

After printing the synthesis, the skill is done. No save, no follow-up, no remora spawn.
