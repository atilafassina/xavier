# Xavier

AI agent orchestrator for Claude Code. Xavier manages code reviews, design interviews, dependency knowledge, and task planning through a personal knowledge vault.

## Prerequisites

- **git** — required for vault initialization and state tracking
- **macOS or Linux**
- **[Claude Code](https://docs.anthropic.com/en/docs/claude-code)** — primary supported runtime

## Installation

```sh
git clone https://github.com/atilafassina/xavier.git
cd xavier
bash install.sh
```

The installer scaffolds a vault at `~/.xavier/`, detects your runtime, wires the adapter, and creates the necessary symlinks. You're ready to go.

## Skills

### Code Review

| Command | Description |
|---------|-------------|
| `/xavier review` | Run Shark-pattern code review on your current diff with 3 concurrent reviewer personas |
| `/xavier babysit` | Monitor a PR, poll CI status, auto-fix lint failures, and surface review comments |

### Design & Planning

| Command | Description |
|---------|-------------|
| `/xavier grill` | Interview you about a plan or design until reaching shared understanding |
| `/xavier prd` | Create a PRD through user interview, codebase exploration, and module design |
| `/xavier tasks` | Decompose a PRD into phased implementation tasks using tracer-bullet slices |

### Dependency Management

| Command | Description |
|---------|-------------|
| `/xavier add-dep <package>` | Create a dependency-skill for a Node package with best practices and API patterns |
| `/xavier remove-dep <package>` | Delete a dependency-skill |
| `/xavier deps-update` | Scan lockfile and regenerate stale dependency-skills |

### Execution

| Command | Description |
|---------|-------------|
| `/xavier loop` | Execute a task file as an autonomous loop using the Shark pattern |

### Vault & Setup

| Command | Description |
|---------|-------------|
| `/xavier setup` | Create and configure the Xavier vault |
| `/xavier export` | Export a vault note to your personal Obsidian vault |
| `/xavier uninstall` | Remove the Xavier vault and all symlinks |

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

Run `bash uninstall.sh` from the repo, or `/xavier uninstall` from Claude Code.
