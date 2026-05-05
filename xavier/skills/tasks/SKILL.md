---
name: tasks
requires: [config, tasks-index, prd-index, repo-conventions, team-conventions]
---

# Tasks

`/xavier tasks`

Decompose a PRD into a phased implementation task list using tracer-bullet vertical slices.

## Step 0: Pre-flight

Check that the PRD list from the resolved `prd-index` context is non-empty (i.e., `~/.xavier/prd/` contains at least one `.md` file). If empty, print: "Error: no PRDs found in ~/.xavier/prd/. Create a PRD first before generating tasks." and stop.

## Step 1: Select PRD

List all `.md` files in `~/.xavier/prd/` (from the resolved `prd-index` context) showing filename, title, date, and tags from frontmatter. Present as a numbered list using AskUserQuestion. If the user already specified a PRD by name, skip the listing and read it directly.

**Soft-resolve fallback for explicit PRD name argument** — When the user invokes the skill with an explicit PRD name (skipping the picker), resolve `<name>` against the four lifecycle cases before reading:

- **Active-only** (file exists at `<vault>/prd/<name>.md`, NOT at `<vault>/prd/done/<name>.md`) → read it directly and proceed.
- **Done-only** (file exists ONLY at `<vault>/prd/done/<name>.md`, no top-level counterpart) → output the revival message and exit cleanly: `PRD <name> is marked done. Revive it with /xavier mark <name> active first, then re-run.` Do NOT continue with task generation.
- **Ambiguous** (file exists at BOTH `<vault>/prd/<name>.md` and `<vault>/prd/done/<name>.md`) → silently prefer the active top-level PRD. Do not emit a revival prompt.
- **Missing** (file exists at NEITHER path) → fall through to the existing "not found" behavior (no revival prompt, no soft-resolve). No behavior change here.

**Soft-resolve fallback for explicit task name argument** — In the rare case that a task name argument is supplied (e.g., when the skill is invoked to operate on or regenerate an existing task file), apply the same four-case resolution against `<vault>/tasks/<name>.md` vs `<vault>/tasks/done/<name>.md`:

- **Active-only** (file exists at `<vault>/tasks/<name>.md`, NOT at `<vault>/tasks/done/<name>.md`) → proceed normally with the active task file.
- **Done-only** (file exists ONLY at `<vault>/tasks/done/<name>.md`, no top-level counterpart) → output the revival message and exit cleanly: `task <name> is marked done. Revive it with /xavier mark <name> active first, then re-run.` Do NOT continue.
- **Ambiguous** (file exists at BOTH `<vault>/tasks/<name>.md` and `<vault>/tasks/done/<name>.md`) → silently prefer the active top-level task file. Do not emit a revival prompt.
- **Missing** (file exists at NEITHER path) → fall through to the existing "not found" behavior (no revival prompt, no soft-resolve). No behavior change here.

## Step 2: Load PRD and Follow Related Links

1. Read the full contents of the selected PRD
2. Check the PRD's `related` field in frontmatter for wikilinks (e.g., `[[prd/auth-middleware]]`, `[[knowledge/teams/platform]]`)
3. **Auto-load** all linked notes that exist in `~/.xavier/`
4. **If 8+ linked notes**: warn the user with a word count estimate and ask whether to load all or pick a subset
5. The loaded context informs the decomposition — understanding prior PRDs, team conventions, and repo knowledge helps produce better slices

## Step 3: Explore Codebase & Detect Backpressure

1. Explore the codebase to understand current architecture, existing patterns, and integration layers
2. Detect backpressure commands using the detection table in `references/patterns/backpressure-detection.md`. Only include commands that actually exist in the project.

## Step 4: Identify Architectural Decisions

Identify durable decisions unlikely to change during implementation: route structures, schema shape, key data models, auth approach, third-party boundaries. These go in the task list header.

## Step 5: Draft Vertical Slices

Break the PRD into tracer bullet phases. Each phase is a thin vertical slice cutting through ALL integration layers end-to-end:

- Each slice delivers a narrow but COMPLETE path through every layer (schema, API, UI, tests)
- A completed slice is demoable or verifiable on its own
- Prefer many thin slices over few thick ones
- Do NOT include specific file names or implementation details likely to change
- DO include durable decisions: route paths, schema shapes, data model names

## Step 6: Quiz the User

Present the proposed breakdown. For each phase show title and user stories covered. Ask:
- Does the granularity feel right?
- Should any phases be merged or split?

Iterate until the user approves.

## Step 7: Write Tasks File

**Before writing**, check if `~/.xavier/tasks/<filename>.md` already exists. If it does, use **AskUserQuestion** to confirm:

> Task file `tasks/{filename}.md` already exists. Overwrite it? (yes/no)

If the user declines, ask for an alternative filename or abort.

Write to `~/.xavier/tasks/<filename>.md` with Zettelkasten frontmatter (see `~/.xavier/references/formats/zettelkasten.md`):

```yaml
---
repo: {current repo name}
type: tasks
source: "[[prd/<filename>]]"
created: {ISO date}
updated: {ISO date}
tags:
  - tasks
  - {feature-related tags}
related:
  - "[[prd/<filename>]]"
---
```

Then write the task body: architectural decisions, backpressure commands, completion criteria, and phases with acceptance criteria.

## Step 8: Offer to Supersede Source PRD

Decomposition is the start of implementation, not the end of it — `done` is **not** offered here. (The source PRD becomes a candidate for `done` only when its derived tasks finish, which happens via `xavier/skills/loop/SKILL.md` Step 6 after the last sibling task is auto-marked done.)

This step exists for one case only: a fresh decomposition that **replaces** an older PRD on the same topic. In that case, the older PRD should be marked `superseded`. Prompt the user via **AskUserQuestion**:

> Does this decomposition replace an older PRD that should be marked `superseded`?

Options: `superseded`, `skip` (default).

Dispatch:

- **`superseded`** → first ask which PRD to supersede (via a follow-up **AskUserQuestion** picker drawn from the resolved `prd-index`, excluding the just-decomposed PRD). Validate the picked basename per the Name Validation rules in `xavier/skills/mark/SKILL.md`. Then apply the `→ superseded` transition from `xavier/skills/mark/SKILL.md` to the chosen PRD. The canonical transition contract — name validation, idempotency, move-precondition, frontmatter-then-mv ordering, and rollback — lives in `mark`; do not duplicate it here.
- **`skip`** → leave all PRDs untouched. No filesystem or frontmatter change.

Do not commit here. The router commits vault changes after the skill completes (mirroring the policy in `mark/SKILL.md`).

## Step 9: STOP — Do not implement

<stop-guardrail>
**You are DONE.** Do not write any code. Do not start implementing any phase.
</stop-guardrail>

Present the user with options:

- **Review first (recommended)**: review the tasks file, then start `/xavier loop` in a fresh conversation
- **Start immediately**: run `/xavier loop` in a clean context

Remind: a clean context reduces token cost and avoids stale exploration state.
