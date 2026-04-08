---
name: tasks
requires: [config, tasks-index, prd-index, repo-conventions, team-conventions]
---

# Tasks

`/xavier tasks`

Decompose a PRD into a phased implementation task list using tracer-bullet vertical slices.

## Step 1: Select PRD

List all `.md` files in `~/.xavier/prd/` (from the resolved `prd-index` context) showing filename, title, date, and tags from frontmatter. Present as a numbered list using AskUserQuestion. If the user already specified a PRD by name, skip the listing and read it directly.

## Step 2: Load PRD and Follow Related Links

1. Read the full contents of the selected PRD
2. Check the PRD's `related` field in frontmatter for wikilinks (e.g., `[[prd/auth-middleware]]`, `[[knowledge/teams/platform]]`)
3. **Auto-load** all linked notes that exist in `~/.xavier/`
4. **If 8+ linked notes**: warn the user with a word count estimate and ask whether to load all or pick a subset
5. The loaded context informs the decomposition — understanding prior PRDs, team conventions, and repo knowledge helps produce better slices

## Step 3: Explore Codebase & Detect Backpressure

1. Explore the codebase to understand current architecture, existing patterns, and integration layers
2. Detect backpressure commands by scanning project root for config files:

| Config file | What to check | Suggested commands |
|---|---|---|
| `package.json` | `scripts` keys (`test`, `build`, `lint`, `typecheck`) | `npm test` / `npm run build` / etc. |
| `Cargo.toml` | presence | `cargo test`, `cargo clippy -- -D warnings` |
| `pyproject.toml` | tools in optional-dependencies | `pytest`, `mypy .`, `ruff check .` |
| `go.mod` | presence | `go test ./...`, `go vet ./...` |
| `Makefile` | targets like `test`, `check`, `lint` | `make test` / `make check` |

Only include commands that actually exist in the project.

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

## Step 8: STOP — Do not implement

<stop-guardrail>
**You are DONE.** Do not write any code. Do not start implementing any phase.
</stop-guardrail>

Present the user with options:

- **Review first (recommended)**: review the tasks file, then start `/xavier loop` in a fresh conversation
- **Start immediately**: run `/xavier loop` in a clean context

Remind: a clean context reduces token cost and avoids stale exploration state.
