---
name: prd
requires: [config, prd-index, repo-conventions, team-conventions]
---

# PRD

`/xavier prd`

Create a vault-aware PRD through user interview, codebase exploration, and module design.

## Step 1: Vault Context Selection

Before the interview begins, present vault contents for the user to browse and select relevant context:

1. **PRD reference resolution (soft-resolve fallback)** — If the user invoked the skill with an explicit PRD name argument (e.g., `/xavier prd <name>`), first **validate `<name>` as a basename** per the Name Validation rules in `xavier/skills/mark/SKILL.md` (must match `^[a-z0-9][a-z0-9-]{0,63}$`). If validation fails, abort before any filesystem check — never let an unvalidated argument reach a path. Then resolve `<name>` against the four lifecycle cases:
   - **Active-only** (file exists at `<vault>/prd/<name>.md`, NOT at `<vault>/prd/done/<name>.md`) → proceed normally with the active PRD as context.
   - **Done-only** (file exists ONLY at `<vault>/prd/done/<name>.md`, no top-level counterpart) → read the file's frontmatter `status` to recover the actual lifecycle state (the directory holds both `done` and `superseded`). Emit the matching revival message — `PRD <name> is marked done. Revive it with /xavier mark <name> active first, then re-run.` if `status: done`, or `PRD <name> is marked superseded. Revive it with /xavier mark <name> active first, then re-run.` if `status: superseded`. If the file lives in `done/` but the status field is missing or invalid, surface a separate warning (the validator should catch this; if it leaks through, point the user at `validate-xavier-frontmatter.sh`). Exit cleanly. Do NOT continue with vault context selection or the interview.
   - **Ambiguous** (file exists at BOTH `<vault>/prd/<name>.md` and `<vault>/prd/done/<name>.md`) → silently prefer the active top-level PRD. Do not emit a revival prompt.
   - **Missing** (file exists at NEITHER path) → fall through to the existing "not found" behavior (no revival prompt, no soft-resolve). No behavior change here.
2. List titles and frontmatter from `~/.xavier/prd/` (from the resolved `prd-index` context), `~/.xavier/knowledge/repos/`, and `~/.xavier/knowledge/teams/`
3. Present as a numbered list using AskUserQuestion (multiSelect: true) — the user picks which notes provide relevant context for the new PRD
4. Read the selected notes and keep their content available for informing interview questions and the final PRD
5. If no notes exist in any of these directories, skip this step silently

## Step 2: Interview

Run the interview flow. The selected vault context informs follow-up questions — reference specific team conventions or prior PRDs where relevant.

1. **Problem statement** — Ask the user for a long, detailed description of the problem they want to solve and any potential ideas for solutions
2. **Codebase exploration** — Explore the repo to verify assertions and understand the current state
3. **Relentless questioning** — Interview the user about every aspect of the plan until reaching shared understanding. Walk down each branch of the design tree, resolving dependencies one-by-one. Use vault context to ask more targeted questions (e.g., "This relates to the auth middleware from your previous PRD — should we build on that or start fresh?")
4. **Module design** — Sketch major modules to build or modify. Prefer deep modules (encapsulate complexity behind simple, testable interfaces). Check with user that modules match expectations. Check which modules need tests
5. **User quiz** — Verify the user agrees with the complete understanding before writing

## Step 3: Write PRD

Write the PRD to `~/.xavier/prd/<filename>.md` where `<filename>` is a kebab-case name derived from the feature. Confirm `<filename>` with the user before writing.

**Validate `<filename>` as a basename before any filesystem write.** It MUST match `^[a-z0-9][a-z0-9-]{0,63}$` per the Name Validation rules in `xavier/skills/mark/SKILL.md` — lowercase letters, digits, hyphens; 1–64 characters; no `/`, `\`, `..`, leading `.`, whitespace, absolute paths, or characters outside `[a-z0-9-]`. If the user-confirmed filename does not match, ask them to provide one that does. The resolved write path MUST be exactly `$XAVIER_HOME/prd/<filename>.md` — no path components from `<filename>` may escape the `prd/` directory.

**Reject collisions with the archive side.** Before writing, also check `$XAVIER_HOME/prd/done/<filename>.md`. If a file already exists there, abort with: `Cannot create PRD '<filename>': an archived PRD with the same basename already exists at <vault>/prd/done/<filename>.md. Pick a different basename, or revive the archived one with '/xavier mark <filename> active' first.` Two files with the same basename across active and `done/` would otherwise leave `/xavier mark` permanently ambiguous on that name.

The PRD uses Zettelkasten frontmatter (see `~/.xavier/references/formats/zettelkasten.md`):

```yaml
---
repo: {current repo name}
team: {from ~/.xavier/config.md}
type: prd
related: [{wikilinks to vault notes selected in Step 1, e.g. "[[prd/auth-middleware]]", "[[knowledge/teams/platform]]"}]
created: {ISO date}
updated: {ISO date}
tags:
  - prd
  - draft
---
```

Then write the PRD body:

- Problem Statement
- Solution
- User Stories (extensive numbered list: "As an X, I want Y, so that Z")
- Implementation Decisions (modules, interfaces, architecture — no file paths or code snippets)
- Testing Decisions (what to test, testing philosophy, prior art)
- Out of Scope
- Further Notes

> **Important**: The PRD is written to `~/.xavier/prd/` only — NOT to the user's Obsidian vault. Use `/xavier export` to sync it there.

Tell the user the PRD was written and remind them they can export it with `/xavier export prd/<filename>`.
