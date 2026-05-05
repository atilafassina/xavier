# Zettelkasten Frontmatter Schema

Base frontmatter schema for all knowledge notes in the Xavier vault. Every note written to `~/.xavier/` (except config, MEMORY.md, and ephemeral state files) should include this frontmatter.

## Required Fields

```yaml
---
repo: {repository name where the note originated}
type: {note type — one of: review, prd, tasks, knowledge, dependency, research, investigation}
created: {ISO date, e.g. 2026-04-05}
updated: {ISO date, e.g. 2026-04-05}
tags:
  - {relevant tags}
related:
  - "[[wikilink to related vault note]]"
---
```

## Field Reference


| Field     | Type   | Description                                                              |
| --------- | ------ | ------------------------------------------------------------------------ |
| `repo`    | string | Name of the git repository this note relates to (optional for `type: research`) |
| `team`    | string | Team name (from config) — optional, include when the note is team-scoped |
| `module`  | string | Most-changed directory or module — optional, include for reviews         |
| `type`    | string | Note type: `review`, `prd`, `tasks`, `knowledge`, `dependency`, `research`, `investigation` |
| `created` | date   | ISO date when the note was first created                                 |
| `updated` | date   | ISO date when the note was last modified                                 |
| `tags`    | list   | Freeform tags for categorization and search                              |
| `related` | list   | Wikilinks (`[[path/name]]`) to other vault notes that provide context    |


## Optional Fields

### `status`

Lifecycle marker for time-bound notes (PRDs, tasks). Allowed values:

- `done` — work is complete; the note has been moved to the `done/` subdirectory
- `superseded` — replaced by another note (typically referenced via `related`)

**Rule: absence of `status` ≡ active.** Notes without a `status` field are treated as live, in-flight work. The router's `prd-index` and `tasks-index` requires keys glob top-level `*.md` only and never recurse into `done/`, so moving a note to `done/` (and setting `status: done`) hides it from active choice lists while preserving full history on disk.

## Type-Specific Fields

### Reviews

- `verdict`: `approve`, `request-changes`, or `rethink`
- `finding-categories`: list of categories found (e.g., `[correctness, security]`)
- `recurring`: list of findings that appeared in past reviews

### PRDs

(No additional type-specific fields beyond the base schema.)

### Tasks

- `source`: wikilink to the originating PRD (e.g., `"[[prd/my-feature]]"`)

### Dependencies

- `version`: package version
- `source`: documentation URL

### Research

- `topic`: the research topic string — primary identifier (research notes are topic-first, not repo-first; `repo` is optional)
- `sources`: list of URLs and file paths consulted by research remoras

### Investigations

- `symptom`: normalized one-line summary of what's broken — primary identifier for investigation notes
- `verdict`: one-line summary of the top-ranked hypothesis — enables scanning investigation notes without reading the full body

## Wikilink Conventions

- Use relative paths within the vault: `[[prd/my-feature]]`, `[[knowledge/teams/platform]]`
- Wikilinks enable Obsidian graph view when notes are exported
- The `related` field is the primary mechanism for linking notes — prefer explicit links over implicit tag-based discovery

