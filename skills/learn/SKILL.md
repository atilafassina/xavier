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

Tell the user the architecture note was created and that the decisions and dependencies remoras are being spawned next.

## Step 4: Decisions Remora

Spawn a background agent to explore the codebase and produce a decisions knowledge note.

```
Agent(
  prompt: """
  You are a technical decisions analyst. Explore the codebase at {repo root} and produce a decisions knowledge note.

  Investigate:
  1. **Framework choices** — what frameworks are used and why they were likely chosen
  2. **Build tool selection** — bundlers, compilers, task runners
  3. **Testing strategy** — frameworks, patterns, coverage approach
  4. **Auth approach** — authentication/authorization strategy (if applicable)
  5. **Deployment patterns** — Dockerfiles, CI config, serverless config
  6. **Data model decisions** — ORM, schema, migrations

  Write the output as a single Markdown file with Zettelkasten frontmatter. Use this exact frontmatter schema:

  ---
  repo: {repo-name}
  type: knowledge
  created: {ISO date}
  updated: {ISO date}
  inferred: true
  tags:
    - decisions
    - {relevant tags}
  related:
    - "[[knowledge/repos/{repo-name}/architecture]]"
    - "[[knowledge/repos/{repo-name}/dependencies]]"
    - "[[knowledge/teams/{team}/conventions]]"
  ---

  All decisions are inferred from the codebase, not confirmed by the team — the `inferred: true` field in the frontmatter marks this.

  After the frontmatter, write the body organized as:

  # Decisions — {repo-name}

  ## {Decision Title}
  **Choice**: what was chosen
  **Alternatives considered**: likely alternatives (inferred)
  **Evidence**: files/patterns that reveal this decision

  ...repeat for each decision discovered...

  Keep it factual and concise. Under 400 lines.
  """,
  description: "xavier learn: decisions remora for {repo-name}",
  run_in_background: true,
  subagent_type: "Explore"
)
```

Once the remora completes, write its output to `<vault>/knowledge/repos/<repo-name>/decisions.md`.

## Step 5: Dependencies Remora

Spawn a background agent to read all dependency manifests and produce a dependencies knowledge note.

```
Agent(
  prompt: """
  You are a dependency analyst. Read all `package.json` files (and any other dependency manifests) in the codebase at {repo root} and produce a comprehensive dependencies note.

  For every direct dependency:
  - Name
  - Version
  - Inferred purpose
  - Consuming modules (which parts of the codebase import/use it)

  For every direct devDependency:
  - Name
  - Version
  - Inferred purpose

  Write the output as a single Markdown file with Zettelkasten frontmatter. Use this exact frontmatter schema:

  ---
  repo: {repo-name}
  type: knowledge
  created: {ISO date}
  updated: {ISO date}
  tags:
    - dependencies
    - {relevant tags}
  related:
    - "[[knowledge/repos/{repo-name}/architecture]]"
    - "[[knowledge/repos/{repo-name}/decisions]]"
    - "[[knowledge/teams/{team}/conventions]]"
  ---

  After the frontmatter, write the body organized as:

  # Dependencies — {repo-name}

  ## Production Dependencies
  | Package | Version | Purpose | Consuming Modules |
  |---------|---------|---------|-------------------|
  | ...     | ...     | ...     | ...               |

  ## Dev Dependencies
  | Package | Version | Purpose |
  |---------|---------|---------|
  | ...     | ...     | ...     |

  Keep it factual and concise. Under 400 lines.
  """,
  description: "xavier learn: dependencies remora for {repo-name}",
  run_in_background: true,
  subagent_type: "Explore"
)
```

Once the remora completes, write its output to `<vault>/knowledge/repos/<repo-name>/dependencies.md`.

## Step 6: Add-dep Delegation

After BOTH the decisions and dependencies remoras from Steps 4 and 5 have completed:

1. Read the generated `<vault>/knowledge/repos/<repo-name>/dependencies.md`.
2. Present the full dependency list to the user.
3. Suggest approximately 5 key dependencies that would benefit from having dependency-skills. Focus on the packages most relevant for code reviews (e.g., core frameworks, state management, testing libraries, API clients).
4. Use `AskUserQuestion` to let the user select which packages they want skills for. The user may pick from your suggestions or type additional package names.
5. For each selected package, delegate to `/xavier add-dep <package-name>`. Do NOT duplicate the add-dep logic inline — invoke the skill directly.
