---
name: prd
requires: [config, prd-index]
---

# PRD

`/xavier prd`

Create a vault-aware PRD through user interview, codebase exploration, and module design.

## Step 1: Vault Context Selection

Before the interview begins, present vault contents for the user to browse and select relevant context:

1. List titles and frontmatter from `~/.xavier/prd/` (from the resolved `prd-index` context), `~/.xavier/knowledge/repos/`, and `~/.xavier/knowledge/teams/`
2. Present as a numbered list using AskUserQuestion (multiSelect: true) — the user picks which notes provide relevant context for the new PRD
3. Read the selected notes and keep their content available for informing interview questions and the final PRD
4. If no notes exist in any of these directories, skip this step silently

## Step 2: Interview

Run the interview flow. The selected vault context informs follow-up questions — reference specific team conventions or prior PRDs where relevant.

1. **Problem statement** — Ask the user for a long, detailed description of the problem they want to solve and any potential ideas for solutions
2. **Codebase exploration** — Explore the repo to verify assertions and understand the current state
3. **Relentless questioning** — Interview the user about every aspect of the plan until reaching shared understanding. Walk down each branch of the design tree, resolving dependencies one-by-one. Use vault context to ask more targeted questions (e.g., "This relates to the auth middleware from your previous PRD — should we build on that or start fresh?")
4. **Module design** — Sketch major modules to build or modify. Prefer deep modules (encapsulate complexity behind simple, testable interfaces). Check with user that modules match expectations. Check which modules need tests
5. **User quiz** — Verify the user agrees with the complete understanding before writing

## Step 3: Write PRD

Write the PRD to `~/.xavier/prd/<filename>.md` where `<filename>` is a kebab-case name derived from the feature. Confirm filename with the user before writing.

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
