---
name: research
description: Research a topic across web, internal docs, and codebase. Produces a structured digest saved to the vault.
requires: [shark, adapter, research-index:optional]
---

# Research

`/xavier research <topic> [--plan]`

Research a topic by spawning parallel remoras across web, internal docs, and codebase. Produces a structured digest presented inline and saved to the vault.

## Step 1: Detect-and-Defer

Follow the detect-and-defer protocol from the Shark reference:

```bash
echo "$SHARK_TASK_HASH"
```

- **If set** (non-empty): act as a simple researcher — do the research inline, skip Shark orchestration.
- **If unset** (empty): proceed as top-level orchestrator with the full flow below.

## Step 2: Parse Input

1. Extract the `--plan` flag if present. The remainder is the topic string.
2. If no topic is provided, ask the user: "What topic would you like to research?"

## Step 3: Check Prior Research

Use the resolved `research-index` context to check for existing research notes on this topic:

1. Glob `research/` for notes with matching or related filenames/topics
2. If a match is found, present the existing note's TL;DR and date, then ask via `AskUserQuestion`: "Update this research or start fresh?"
   - **Update**: read the full prior note. Its content becomes additional context for each remora prompt.
   - **Start fresh**: proceed without prior context. The new note overwrites the old one.
3. If no match is found, proceed normally.

## Step 4: Decompose Topic

Generate 3-5 research questions using the guided template:

**Fixed axes (always present):**
- **Foundations** — What is this concept? Core principles, mental model, key terminology.
- **Practice** — How is it used in industry? Tools, patterns, common approaches, tradeoffs.
- **State of Art** — What are the latest developments? Competing approaches, where the field is heading.

**Dynamic axes (1-2, topic-specific):**
- Analyze the topic and generate 1-2 additional research questions that target what's most interesting or nuanced about this specific topic. Examples: "security implications" for an auth topic, "performance characteristics" for a data structure, "ecosystem comparison" for a framework.

**Local Context (conditional):**
- Only include if the current working directory is a git repo (check with `git rev-parse --git-dir`).
- Question: "How does the current codebase connect to {topic}? Look for related files, modules, patterns, implementations, dependencies, or documentation."

## Step 5: Plan Gate

If the `--plan` flag was set:

1. Present the decomposed questions as a numbered list via `AskUserQuestion`
2. The user can approve, edit, remove, or add questions
3. Proceed with the approved set of questions

If no `--plan` flag, skip this step and proceed immediately.

## Step 6: Spawn Research Remoras

Spawn one remora per research question via adapter `collect()` — all in a **single message** with parallel tool calls using `run_in_background: true`.

**Concept remora prompt template:**

```
Export SHARK_TASK_HASH={hash} before starting work.

Research the following question about "{topic}":

{research question}

Use any tools available to find the best answer: WebSearch, WebFetch, Glean (internal docs search), Confluence, codebase search (Grep, Glob, Read).

{if augmenting: "Prior research on this topic is provided below for context. Focus on what's new, changed, or was missed:\n\n{prior note content}"}

Constraints:
- Return a concise, factual answer under 500 words
- End with a ### Sources subsection listing every URL and file path you consulted
- Do NOT make recommendations — report what you find
- Do NOT spawn sub-agents
```

**Local context remora prompt template:**

```
Export SHARK_TASK_HASH={hash} before starting work.

Explore the codebase at {cwd} to find connections to "{topic}".

Look for:
- Files, modules, or patterns related to this topic
- Existing implementations or references
- Configuration or dependencies that touch this domain
- Comments or documentation mentioning related concepts

{if augmenting: "Prior research found these local connections: {prior local context}. Focus on what's changed or was missed."}

Constraints:
- Return a concise factual summary under 500 words
- End with a ### Sources subsection listing file paths consulted
- Do NOT make recommendations — report what you find
- Do NOT spawn sub-agents
```

All remoras use `subagent_type: "general-purpose"`.

## Step 7: Collect and Synthesize

As each remora completes, record its findings. Once all have reported, synthesize a digest:

1. **TL;DR** — 3-5 sentences summarizing the topic
2. **Sections** — One per research axis. Do NOT copy-paste remora output — restructure, deduplicate, and connect findings across axes.
3. **Sources** — Merge and deduplicate all sources from all remoras. Format: `[Title](url) — one-liner` for web sources, `` `path/to/file` — what it shows `` for codebase sources.

## Step 8: Present and Save

1. **Present the digest inline** in the conversation.
2. **Suggest a filename**: derive a kebab-case name from the topic (e.g., "semantic modelling for BI" → `semantic-modelling-for-bi.md`). Confirm with the user.
3. **Create directory if needed**: `mkdir -p` on `research/` in the vault if it doesn't exist.
4. **Save to vault**: Write the note to `research/<filename>.md` with Zettelkasten frontmatter:

```yaml
---
topic: "{the topic string}"
repo: {current repo name, or omit if not in a git repo}
type: research
created: {ISO date}
updated: {ISO date}
tags:
  - research
  - {topic-derived tags}
related:
  - "[[research/prior-note-if-augmenting]]"
sources:
  - "{url or file path}"
  - "{url or file path}"
---
```

Then the digest body:

```markdown
## TL;DR

3-5 sentences.

## Foundations

Core concepts, principles, mental model.

## Practice

Industry usage, tools, patterns, tradeoffs.

## State of Art

Latest developments, competing approaches.

## {Dynamic Axis Title}

Topic-specific deep dive.

## Local Context

(Only present if local context remora was spawned)

## Sources

- [Title](url) — one-line description
- `path/to/file` — what it shows
```

Tell the user the research note was saved and remind them they can export it with `/xavier export research/<filename>`.
