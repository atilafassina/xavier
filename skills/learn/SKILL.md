---
name: learn
requires: [config, shark, adapter, repo-conventions, team-conventions]
---

# Learn

`/xavier learn`

Explore a codebase and produce knowledge notes in the Xavier vault. Detects the current repository, resolves team ownership, and spawns a background agent to map the architecture into Zettelkasten-style notes.

## Step 1: Re-run Guard

1. Resolve the repository name:

```bash
basename $(git rev-parse --show-toplevel)
```

2. Check if `<vault>/knowledge/repos/<repo-name>/` already contains notes (look for any `.md` files in the directory).
3. **If notes exist**: Warn the user that knowledge notes already exist for this repo. Use `AskUserQuestion` to confirm whether to overwrite or abort.
   - If the user chooses to abort, stop execution and inform them that no changes were made.
   - If the user chooses to overwrite, proceed to Step 2.
4. **If no notes exist**: Proceed to Step 2.

## Step 2: Team Resolution

1. Read the `teams` list from the resolved `config` context.
2. Present the list of known teams to the user. Use `AskUserQuestion` to ask which team owns this repository. The user may pick an existing team or type a new team name.
3. **If the user picks an existing team**: Record the team name for use in Step 3.
4. **If the user types a new team name**:
   - Append the new team to the `teams` list in config.
   - Create `<vault>/knowledge/teams/<team>/conventions.md` with this stub content:

```markdown
---
repo: (shared)
type: knowledge
created: {ISO date}
updated: {ISO date}
tags:
  - team
  - conventions
related:
  - "[[knowledge/teams/{team}]]"
---

# {team} Conventions

<!-- Add team-wide conventions, patterns, and guidelines here -->
```

5. The repo notes created in Step 3 must include a wikilink back to the team: `[[knowledge/teams/<team>/conventions]]`.

## Step 3: Architecture Remora

Spawn a background agent to explore the codebase and produce an architecture knowledge note.

```
Agent(
  prompt: """
  You are an architecture analyst. Explore the codebase at {repo root} and produce a comprehensive architecture note.

  Investigate:
  1. **Stack summary** — languages, frameworks, build tools, runtime
  2. **Module map** — top-level directory structure and what each module/package does
  3. **Entry points** — main files, CLI entry points, server start files, handler roots
  4. **Key patterns** — architectural patterns used (MVC, event-driven, plugin system, etc.), state management, error handling conventions
  5. **Integration boundaries** — external services, APIs consumed/exposed, database connections, message queues

  Write the output as a single Markdown file with Zettelkasten frontmatter. Use this exact frontmatter schema:

  ---
  repo: {repo-name}
  type: knowledge
  created: {ISO date}
  updated: {ISO date}
  tags:
    - architecture
    - {stack-related tags}
  related:
    - "[[knowledge/repos/{repo-name}/decisions]]"
    - "[[knowledge/repos/{repo-name}/dependencies]]"
    - "[[knowledge/teams/{team}/conventions]]"
  ---

  Note: decisions.md and dependencies.md do not exist yet — include the wikilinks anyway so the vault graph connects them when they are created later.

  After the frontmatter, write the body organized under these headings:
  # Architecture — {repo-name}
  ## Stack
  ## Modules
  ## Entry Points
  ## Key Patterns
  ## Integration Boundaries

  Keep it factual and concise. Under 400 lines.
  """,
  description: "xavier learn: architecture remora for {repo-name}",
  run_in_background: true,
  subagent_type: "Explore"
)
```

Once the remora completes, write its output to `<vault>/knowledge/repos/<repo-name>/architecture.md`.

Tell the user the architecture note was created and suggest next steps:
- Review `[[knowledge/repos/<repo-name>/architecture]]`
- Future skills will populate `[[knowledge/repos/<repo-name>/decisions]]` and `[[knowledge/repos/<repo-name>/dependencies]]`
