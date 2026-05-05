---
name: tasks
requires: [config, tasks-index, prd-index, repo-conventions, team-conventions]
---

# Tasks

`/xavier tasks`

Decompose a PRD into a phased implementation task list using tracer-bullet vertical slices.

## Step 0: Pre-flight

Branch on whether the user supplied an explicit PRD name argument:

- **Explicit name argument given** → skip the empty-index check and proceed to Step 1's soft-resolve fallback. The name may resolve under `prd/done/` even when no active PRDs exist, and the soft-resolve fallback is what surfaces the revival hint in that case. Bailing here would suppress the hint and leave the user with a misleading "no PRDs found" error.
- **No name argument (picker flow)** → check that the PRD list from the resolved `prd-index` context is non-empty (i.e., `~/.xavier/prd/` contains at least one active `.md` file). If empty, print: `Error: no active PRDs found in ~/.xavier/prd/. Create a PRD first before generating tasks (or revive an archived one with /xavier mark <name> active).` and stop.

## Step 1: Select PRD

List all `.md` files in `~/.xavier/prd/` (from the resolved `prd-index` context) showing filename, title, date, and tags from frontmatter. Present as a numbered list using AskUserQuestion. If the user already specified a PRD by name, skip the listing and read it directly.

**Soft-resolve fallback for explicit PRD name argument** — When the user invokes the skill with an explicit PRD name (skipping the picker), first **validate `<name>` as a basename** per the Name Validation rules in `xavier/skills/mark/SKILL.md` (must match `^[a-z0-9][a-z0-9-]{0,63}$`). If validation fails, abort with the same error message before any filesystem check — never let an unvalidated argument reach a path. Then resolve `<name>` against the four lifecycle cases:

- **Active-only** (file exists at `<vault>/prd/<name>.md`, NOT at `<vault>/prd/done/<name>.md`) → read it directly and proceed.
- **Done-only** (file exists ONLY at `<vault>/prd/done/<name>.md`, no top-level counterpart) → read the file's frontmatter `status` to recover the actual lifecycle state (the directory holds both `done` and `superseded`). Emit the matching revival message:
  - If `status: done`: `PRD <name> is marked done. Revive it before re-running.`
  - If `status: superseded`: `PRD <name> is marked superseded. Revive it before re-running.`

  Then suggest the recovery path with cross-kind ambiguity awareness: if `<vault>/tasks/<name>.md` or `<vault>/tasks/done/<name>.md` also exists, `/xavier mark <name> active` would hit `mark`'s cross-kind ambiguity error, so suggest the picker form: `Run /xavier mark (no args), select prd/<name>, and choose 'active'. Then re-run.` Otherwise suggest the arg form: `Run /xavier mark <name> active, then re-run.` Exit cleanly. Do NOT continue with task generation.
- **Ambiguous** (file exists at BOTH `<vault>/prd/<name>.md` and `<vault>/prd/done/<name>.md`) → silently prefer the active top-level PRD. Do not emit a revival prompt.
- **Missing** (file exists at NEITHER path) → fall through to the existing "not found" behavior (no revival prompt, no soft-resolve). No behavior change here.

**Reject explicit task name arguments.** This skill operates on PRDs, not tasks — the rest of the body (Steps 2-7) reads the resolved file as a PRD and writes a new task file derived from it. If a caller passes what looks like a task basename instead of a PRD basename, abort with: `/xavier tasks operates on PRDs, not tasks. To regenerate or modify an existing task file, edit it directly or run /xavier mark <name> active first if the source PRD is archived. Aborting.` The skill's frontmatter does not declare any task-write behavior on existing task files, and silently treating a task argument as a PRD would generate the wrong output without surfacing the error.

## Step 2: Load PRD and Follow Related Links

1. Read the full contents of the selected PRD
2. Check the PRD's `related` field in frontmatter for wikilinks (e.g., `[[prd/auth-middleware]]`, `[[knowledge/teams/platform]]`)
3. **Validate every wikilink before any filesystem read.** A wikilink target has the form `[[<namespace>/<path>]]` where:
   - `<namespace>` is the **prefix** that must match one of the approved values:
     - **Single-segment leaf** (exactly one basename after the namespace): `prd`, `prd/done`, `tasks`, `tasks/done`, `knowledge/reviews`, `research`, `investigations`, `deps`.
     - **Multi-segment leaf allowed** (the namespace can be followed by 1 to 3 basename segments to accommodate per-repo / per-package notes written by `/xavier learn`): `knowledge/repos`, `knowledge/teams`. For example `[[knowledge/repos/<repo>/<package>/dependencies]]` is a legal four-segment path under the `knowledge/repos` namespace.
     - Reject anything whose namespace is not in this list.
   - **Every** path segment after the namespace MUST independently match the basename allowlist `^[a-z0-9][a-z0-9-]{0,63}$` from `xavier/skills/mark/SKILL.md`. The full path under the namespace must be 1–4 such segments — no deeper nesting, no empty segments, no segment containing `..`, leading `.`, absolute paths, whitespace, or characters outside `[a-z0-9-]`.
   - The resolved filesystem path MUST canonicalize (via `realpath` or equivalent) to a child of `$XAVIER_HOME` — never auto-load a path that escapes the vault root, even if it textually appears to.
   Log a warning naming any wikilink that fails validation; skip it and continue with the remaining links.
4. **Auto-load** the validated linked notes. The resolved path under `$XAVIER_HOME/<wikilink-target>.md` may point at either a file or a directory:
   - **File** (`<target>.md` exists and is a regular file) → read its full contents.
   - **Directory** (`<target>` exists as a directory) → some Xavier note conventions use directory-style links (e.g., `/xavier learn` writes `[[knowledge/teams/<team>]]` even though the actual note lives at `knowledge/teams/<team>/conventions.md`). For each directory-style target, look for a single conventional note file inside, in this priority order: `<target>/conventions.md`, `<target>/architecture.md`, `<target>/dependencies.md`, `<target>/decisions.md`. Read the first one that exists. If none exist, skip the link with a warning naming the directory.
   - **Missing** (neither file nor directory) → skip silently. Missing wikilink targets are not an error.
   Never read more than one note per wikilink — if a directory holds multiple convention files, the priority list is authoritative.
5. **If 8+ linked notes**: warn the user with a word count estimate and ask whether to load all or pick a subset
6. The loaded context informs the decomposition — understanding prior PRDs, team conventions, and repo knowledge helps produce better slices

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

**Before writing**, check both `~/.xavier/tasks/<task-filename>.md` and `~/.xavier/tasks/done/<task-filename>.md`:

- If `~/.xavier/tasks/<task-filename>.md` exists, use **AskUserQuestion** to confirm:

  > Task file `tasks/{task-filename}.md` already exists. Overwrite it? (yes/no)

  If the user declines, ask for an alternative filename or abort.

- If `~/.xavier/tasks/done/<task-filename>.md` exists (the archive side), abort with: `Cannot create task '<task-filename>': an archived task with the same basename already exists at <vault>/tasks/done/<task-filename>.md. Pick a different basename, or revive the archived one with '/xavier mark <task-filename> active' first.` Allowing a write here would create an active+archived basename collision that `/xavier mark` arg mode refuses to resolve.

Write to `~/.xavier/tasks/<task-filename>.md` with Zettelkasten frontmatter (see `~/.xavier/references/formats/zettelkasten.md`).

The `source` and `related` wikilinks must point to the **source PRD's basename** (chosen in Step 1), not the task file's own filename. These can differ — task filenames may add a feature qualifier (`prd-foo` → `prd-foo-tasks`) or the user may pick a different filename in this step. Treat `<prd-basename>` and `<task-filename>` as independent variables; downstream sibling-scan logic in `xavier/skills/loop/SKILL.md` Step 6 and `xavier/skills/mark/SKILL.md` sub-phase 5b parses `<prd-basename>` out of the `source` wikilink to find the PRD.

```yaml
---
repo: {current repo name}
type: tasks
source: "[[prd/<prd-basename>]]"
created: {ISO date}
updated: {ISO date}
tags:
  - tasks
  - {feature-related tags}
related:
  - "[[prd/<prd-basename>]]"
---
```

Both `<prd-basename>` and `<task-filename>` must satisfy the basename allowlist (`^[a-z0-9][a-z0-9-]{0,63}$`) — validate before writing the file. If the user-suggested filename does not match, ask them to provide one that does.

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
