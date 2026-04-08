---
name: learn
requires: [config, shark, adapter, repo-conventions, team-conventions]
---

# Learn

`/xavier learn`

Explore a codebase and produce knowledge notes in the Xavier vault. Detects the current repository, resolves team ownership, and spawns background agents to map the architecture, decisions, and dependencies into Zettelkasten-style notes.

## Step 1: Re-run Guard

1. Resolve the repository name:

```bash
basename $(git rev-parse --show-toplevel)
```

2. Check the root `package.json` for a `workspaces` field. If present (non-empty array), set a `monorepo: true` flag that affects Steps 4 and 7. If absent or empty, set `monorepo: false`.
3. Check if `<vault>/knowledge/repos/<repo-name>/` already contains notes (look for any `.md` files in the directory).
4. **If notes exist**: Warn the user that knowledge notes already exist for this repo. Use `AskUserQuestion` to confirm whether to overwrite or abort.
   - If the user chooses to abort, stop execution and inform them that no changes were made.
   - If the user chooses to overwrite, proceed to Step 2.
5. **If no notes exist**: Proceed to Step 2.

## Step 2: Team Resolution

1. Read the `teams` list from the resolved `config` context.
2. Present the list of known teams to the user. Use `AskUserQuestion` to ask which team owns this repository. The user may pick an existing team or type a new team name.
3. **If the user picks an existing team**: Record the team name for use in later steps.
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

5. The repo notes created in Step 4 must include a wikilink back to the team: `[[knowledge/teams/<team>/conventions]]`.

## Step 3: Detect-and-Defer

Check the `SHARK_TASK_HASH` environment variable:

```bash
echo "$SHARK_TASK_HASH"
```

- **If set** (non-empty): this agent is running inside an outer Shark loop. Do NOT start the Shark flow. Instead, run the designated remora inline (architecture, decisions, or dependencies based on the task context) and return results directly to the caller. Skip Steps 4-6.
- **If unset** (empty): this agent is the top-level orchestrator. Proceed with the full Shark flow starting at Step 4.

## Step 4: Spawn Research Remoras

> **IMPORTANT**: Spawn all 3 research remoras concurrently in a **single message** with parallel tool calls. All three MUST use `run_in_background: true`.

### Architecture Remora

```
Agent(
  prompt: """
  You are an architecture analyst. Explore the codebase at {repo root} and produce a comprehensive architecture note.

  Investigate:
  1. **Stack summary** — languages, frameworks, build tools, runtime
  2. **Module map** — top-level directory structure and what each module/package does. **In monorepo mode**: use workspace packages (from the `workspaces` field in root `package.json`) as the primary modules instead of top-level src directories.
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

### Decisions Remora

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

### Dependencies Remora

```
Agent(
  prompt: """
  You are a dependency analyst. Read all `package.json` files (and any other dependency manifests) in the codebase at {repo root} and produce a comprehensive dependencies note. **In single-package mode**: read the root `package.json`. **In monorepo mode**: focus on root-level shared dependencies here; per-workspace dependencies will be handled separately in Step 7.

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

## Step 5: Pilot Fish (Progressive Note Writing)

As each remora completes, process its output immediately — do not wait for all remoras:

1. Read the remora output
2. Write the note to `<vault>/knowledge/repos/<repo-name>/` (architecture.md, decisions.md, or dependencies.md)
3. Update the user on progress: "Note 1/3 complete (architecture)..." / "Note 2/3 complete (decisions)..." / "Note 3/3 complete (dependencies)..."
4. After each note is written, update wikilinks in any previously-written sibling notes to ensure cross-references are accurate

After all 3 remoras have completed and all notes are written, proceed to Step 6.

## Step 6: Add-dep Delegation

After all 3 research remoras from Step 4 have completed and all notes are written:

1. Read the generated `<vault>/knowledge/repos/<repo-name>/dependencies.md`.
2. Present the full dependency list to the user.
3. Suggest approximately 5 key dependencies that would benefit from having dependency-skills. Focus on the packages most relevant for code reviews (e.g., core frameworks, state management, testing libraries, API clients).
4. Use `AskUserQuestion` to let the user select which packages they want skills for. The user may pick from your suggestions or type additional package names.
5. For each selected package, delegate to `/xavier add-dep <package-name>`. Do NOT duplicate the add-dep logic inline — invoke the skill directly.

## Step 7: Monorepo Workspace Dependencies

> This step only runs if the `monorepo` flag was set in Step 1. For single-package repos, skip this step entirely.

1. Read the root `package.json` and extract the `workspaces` field (array of glob patterns).
2. Resolve workspace patterns to actual package directories (e.g., `packages/*` → `packages/foo`, `packages/bar`).
3. For each workspace package that has its own `package.json`:
   - Spawn a background agent to read the package's `package.json` and produce a per-workspace dependencies note. Use `run_in_background: true` and spawn all workspace agents concurrently in a **single message** with parallel tool calls.
   - Write the note to `<vault>/knowledge/repos/<repo-name>/<package-name>/dependencies.md`
   - Use the same Zettelkasten frontmatter schema as the repo-level dependencies note, but with the `module` field set to the package name:

   ```yaml
   ---
   repo: {repo-name}
   module: {package-name}
   type: knowledge
   created: {ISO date}
   updated: {ISO date}
   tags:
     - dependencies
     - workspace
     - {package-name}
   related:
     - "[[knowledge/repos/{repo-name}/dependencies]]"
     - "[[knowledge/repos/{repo-name}/architecture]]"
   ---
   ```

4. After all workspace dependency notes are written, update the repo-level `dependencies.md` to add wikilinks to each workspace dependencies note in its `related` frontmatter (e.g., `"[[knowledge/repos/{repo-name}/{package-name}/dependencies]]"` for each workspace package).
