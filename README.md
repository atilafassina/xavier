# Xavier

Self-evolving AI orchestrator.

🔮 codebase **exploration** | dependency **knowledge** | design **interviews** | task **planning** |code **reviews**

## Installation

### Prerequisites

- **git** — required for vault initialization and state tracking
- POSIX (MacOS, Linux, or Windows WSL)
- **[GitHub CLI (gh)](https://cli.github.com/)**
- At least one supported AI agent runtime (see [Supported Runtimes](#supported-runtimes))

### Quick Install

Download and install in one command:

```sh
curl -fsSL https://github.com/atilafassina/xavier/releases/latest/download/xavier.tar.gz | tar xz && bash xavier/install.sh
```

This extracts the tarball and copies skills and references into your vault. No persistent clone needed and works perfectly with `/xavier self-update`.

### Install from source

Clone the repo for a development setup with live symlinks:

```sh
git clone https://github.com/atilafassina/xavier.git
cd xavier
bash xavier/install.sh
```

When installed from source, skills and references are symlinked back to the repo so changes are reflected immediately.

### Supported Runtimes

The installer auto-detects all available runtimes and wires adapters for each. If multiple runtimes are found, the first detected is set as the primary adapter in `config.md` — you can switch by editing the `adapter` field.

| Runtime | Status | Detection | Adapter |
|---------|--------|-----------|---------|
| **[Claude Code](https://docs.anthropic.com/en/docs/claude-code)** | Full support | `claude` on PATH | `Agent` + `Bash` tools |
| **[Cursor](https://www.cursor.com/)** | Full support | `cursor` on PATH | `Task` + `Shell` tools |
| **Codex** | Stub | `codex` on PATH | Not yet available |

Both Claude Code and Cursor can be installed simultaneously — Xavier registers skills in the appropriate paths for each runtime (`~/.claude/commands/` for Claude Code, `~/.cursor/skills/` for Cursor).

## How It Works

 Xavier follows the **Shark pattern**: a central orchestrator that delegates work to concurrent background agents (remoras), never implementing
 anything itself. Results are verified through backpressure — only test, lint, and typecheck output counts as truth.

 A runtime **adapter** layer abstracts the differences between AI agent runtimes. Skills use generic operations (`spawn`, `collect`, `poll`) that the adapter maps to the correct tool for the active runtime — `Agent` for Claude Code, `Task` for Cursor. This means all 14 skills work identically regardless of which runtime you use.

 Three pillars drive every Xavier workflow:

### Personas — Concurrent Specialized Reviewers

 When you run `/xavier review`, Xavier spawns **3 reviewer agents in parallel**, each examining your diff through a different lens:

- **Correctness** — bugs, logic errors, edge cases, type safety
- **Security** — injection, auth, data exposure, CWE references
- **Performance** — algorithmic complexity, memory, I/O, bundle size

 All three receive the same diff but review independently. Findings are deduplicated, ranked by severity, and synthesized into a single verdict
 (`approve`, `request changes`, or `rethink`). Personas can be customized per-repo by adding `.xavier/personas/` to your project root.

### Learning — Codebase Exploration Agents

 `/xavier learn` spawns **3 research remoras concurrently** to map an unfamiliar codebase:

- **Architecture** — modules, entry points, key patterns, integration boundaries
- **Decisions** — framework choices, testing strategy, auth, deployment patterns
- **Dependencies** — all direct/dev packages with consuming modules

 Notes are written progressively as each remora completes (pilot fish pattern). Monorepos are detected automatically, with per-workspace
 analysis. After learning, Xavier suggests key packages for dedicated dependency-skills (`/xavier add-dep`).

### Knowledge Base

 Everything Xavier discovers lives in `~/.xavier/` as interconnected Markdown notes:

 Notes use standardized frontmatter (`repo`, `type`, `tags`, `related` wikilinks) and link to each other for cross-referencing. Review notes
 feed an **active learning loop** — recurring patterns from your last 10 reviews are extracted and injected into future reviewer prompts, so
 Xavier gets sharper over time. The vault is git-tracked and can be exported to Obsidian via `/xavier export`.

## Skills

### Code Review


| Command           | Description                                                                            |
| ----------------- | -------------------------------------------------------------------------------------- |
| `/xavier review`  | Run Shark-pattern code review on your current diff with 3 concurrent reviewer personas |
| `/xavier babysit` | Monitor a PR, poll CI status, auto-fix lint failures, and surface review comments      |


### Design & Planning


| Command         | Description                                                                  |
| --------------- | ---------------------------------------------------------------------------- |
| `/xavier grill` | Interview you about a plan or design until reaching shared understanding     |
| `/xavier prd`   | Create a PRD through user interview, codebase exploration, and module design |
| `/xavier tasks` | Decompose a PRD into phased implementation tasks using tracer-bullet slices  |


### Knowledge


| Command         | Description                                                        |
| --------------- | ------------------------------------------------------------------ |
| `/xavier learn` | Explore a codebase and produce knowledge notes in the Xavier vault |


### Dependency Management


| Command                        | Description                                                                       |
| ------------------------------ | --------------------------------------------------------------------------------- |
| `/xavier add-dep <package>`    | Create a dependency-skill for a Node package with best practices and API patterns |
| `/xavier remove-dep <package>` | Delete a dependency-skill                                                         |
| `/xavier deps-update`          | Scan lockfile and regenerate stale dependency-skills                              |


### Execution


| Command        | Description                                                       |
| -------------- | ----------------------------------------------------------------- |
| `/xavier loop` | Execute a task file as an autonomous loop using the Shark pattern |


### Vault & Setup


| Command               | Description                                               |
| --------------------- | --------------------------------------------------------- |
| `/xavier setup`       | Create and configure the Xavier vault                     |
| `/xavier self-update` | Update Xavier skills and references to the latest release |
| `/xavier export`      | Export a vault note to your personal Obsidian vault       |
| `/xavier uninstall`   | Remove the Xavier vault and all symlinks                  |


## Usage

A typical workflow from idea to implementation:

```
# 1. Grill your design — Xavier interviews you until the plan is solid
/xavier grill

# 2. Turn the grilled design into a PRD
/xavier prd

# 3. Break the PRD into phased tasks
/xavier tasks

# 4. Execute tasks autonomously
/xavier loop
```

## Advanced Usage

### Custom vault location

Set `XAVIER_HOME` to override the default `~/.xavier/` vault path:

```sh
export XAVIER_HOME="$HOME/.config/xavier"
bash install.sh
```

All Xavier commands will use this location when the variable is set.

### Vault structure

The vault maintains your configuration, knowledge, and state:

```
~/.xavier/
├── config.md              # User preferences, adapter, git strategy
├── MEMORY.md              # Learning index
├── knowledge/             # Reviews, repo conventions, team patterns
├── prd/                   # Product requirement documents
├── tasks/                 # Implementation task files
├── references/            # Shared patterns, personas, adapters
├── skills/                # Symlinks to skill definitions
└── *-state/               # Runtime state (loop, review, shark)
```

## Previous Work

Xavier builds on ideas and patterns from these open-source projects:

- **[Shark](https://github.com/keugenek/shark)** by Evgeny Knyazev — the non-blocking execution pattern that keeps agents productive while tools run in the background. Xavier's review and loop skills use the Shark pattern.
- **[Skills](https://github.com/mattpocock/skills)** by Matt Pocock — a collection of reusable agent skills for planning, development, and tooling workflows. Xavier's skill architecture draws from this work.

## Uninstall

Run `bash uninstall.sh` from the repo, or `/xavier uninstall` from your AI agent. The uninstaller removes symlinks for all runtimes (Claude Code, Cursor) and optionally deletes the vault.