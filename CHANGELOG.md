# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `prose-trigger` — opt-in feature that teaches supported runtimes to recognise a vocative invocation of a configurable trigger word ("Xavier" by default) in natural prose and route it through `/xavier <subcommand>`. Disabled by default; enabled through the setup interview or by setting `prose-trigger: yes` in `~/.xavier/config.md`. Supports four routing modes: explicit subcommand keyword → direct invoke; clear-but-implicit intent → one-line confirm before invoke; meta question about Xavier → `/xavier` (router lists subcommands); off-topic prose → drop the trigger and answer normally. **Claude Code:** managed instruction block in `~/.claude/CLAUDE.md` (always loaded). **Cursor:** global skill at `~/.cursor/skills/prose-trigger/SKILL.md` (selection-based; fixed name outside the alias prefix so `/x-` autocomplete stays clean)
- Two new keys in `~/.xavier/config.md` under `## Runtime`: `prose-trigger: yes|no` (default `no`) and `trigger-word: <word>` (default `Xavier`, validated against `^[a-zA-Z][a-zA-Z0-9-]{0,31}$`)
- Two new questions in the `/xavier setup` interview, adjacent to the existing alias prompts — enable prose-trigger (default `no`), and follow-up trigger word (conditional on enable, default `Xavier`)
- `install.sh` gains `install_prose_trigger()` — writes Claude Code managed block to `~/.claude/CLAUDE.md` and Cursor global skill to `~/.cursor/skills/prose-trigger/SKILL.md` when enabled. Idempotent refresh on both surfaces; disable/uninstall strips both
- `install.sh` gains `install_cursor_prose_trigger_skill()` and `strip_cursor_prose_trigger_skill()` — Cursor fallback using router lifecycle delegation (not the Skill tool). Skill name is fixed as `prose-trigger`, decoupled from `alias-prefix`
- `install.sh` gains `strip_prose_trigger_block()` — removes the managed block (markers included) while preserving surrounding user content byte-for-byte; deletes the host file if Xavier was the sole writer. Wired into `install_prose_trigger()` for the disable path and into the uninstall flow
- `/xavier self-update` Step 8b ("Refresh Prose-Trigger Managed Block") — mirrors Claude `~/.claude/CLAUDE.md` write/strip byte-for-byte
- `/xavier self-update` Step 8c ("Refresh Cursor Prose-Trigger Skill") — mirrors `install_cursor_prose_trigger_skill()` byte-for-byte
- `/xavier uninstall` Step 3 ("Strip Prose-Trigger Managed Block") — removes the Claude block from `~/.claude/CLAUDE.md`
- `/xavier uninstall` Step 3b — removes `~/.cursor/skills/prose-trigger/`; `uninstall.sh` removes the same path explicitly
- `validate-skills.sh` marker-drift check — Claude BEGIN/END markers byte-identical in `install.sh` and `self-update/SKILL.md`
- `validate-skills.sh` Cursor prose-trigger template drift check — anchor strings byte-identical in `install.sh` and `self-update/SKILL.md`

### Changed

- The canonical `COMMANDS` list in `install.sh` is now a top-level constant rather than defined inside `install_command_aliases()`; both that function and the new `install_prose_trigger()` consume it
- README documents prose-trigger per-runtime delivery (Claude always-on block vs Cursor selection-based skill), alias-prefix slash-menu isolation, and optional User Rules paste for always-on Cursor behaviour
- `/xavier setup` interview copy updated to describe both Claude Code and Cursor surfaces when prose-trigger is enabled

### Deprecated

### Removed

### Fixed

### Security

## [0.5.0] - 2026-05-07

### Added

- Lifecycle states for PRDs and tasks: optional `status` frontmatter field (`done` | `superseded`) plus a `<vault>/<kind>/done/` subdirectory layout, scaffolded by the installer for both `prd/` and `tasks/`
- `/xavier mark` skill — manually transition PRDs and tasks between `active`, `done`, and `superseded`. Supports a no-arg picker mode (multi-select via `AskUserQuestion`), a two-arg mode (`/xavier mark <name> <state>`), and a one-arg mode that pre-filters the picker to entries whose basename equals the argument (zero matches → not-found error; exactly one → auto-prompt for state; more than one → multi-select disambiguation)
- `/xavier mark --backfill` — one-shot migration for vaults that predate the lifecycle feature. Runs three independently abortable sub-phases: (5a) auto-batch tasks with completed loop-state evidence, (5b) PRD inference for PRDs whose every derived task is now done, (5c) manual sweep with a metadata-rich multi-select picker. Idempotent — re-running yields no additional moves
- Auto-mark hook in `/xavier loop` Step 5 — when every phase passes, the loop silently transitions the source task to `done` via the canonical `→ done` transition in the mark skill. Loop-state files now also gain a stable `status: complete` marker line for backfill detection
- Sibling-scan PRD prompt in `/xavier loop` Step 6 — after a successful loop auto-marks its task, the loop checks whether every sibling task pointing at the same source PRD is now done; if so, prompts the user to mark the PRD as `done`, `superseded`, or `skip`
- Post-decompose PRD prompt in `/xavier tasks` — after writing the new task file, asks whether this decomposition replaces an older PRD (offering `superseded` or `skip`); `done` is intentionally not offered here because decomposition starts implementation rather than finishing it. The "implementation done" path is owned by `/xavier loop` Step 6 after the last sibling task is auto-marked.
- Soft-resolve fallback in `/xavier prd` and `/xavier tasks` — when a name argument resolves only inside `done/`, the skill emits a revival hint (`<noun> <name> is marked done. Revive it with /xavier mark <name> active first, then re-run.`) and exits cleanly, instead of failing with "not found"

### Changed

- Frontmatter validator (`validate-xavier-frontmatter.sh`) now recognizes the optional `status` field and enforces its allowed values
- `prd-index` and `tasks-index` contexts continue to surface only top-level (active) items; archived items in `done/` are reached via direct filesystem globs in the mark skill picker

### Deprecated

### Removed

### Fixed

### Security

## [0.4.0] - 2026-04-23

### Added

- `/xavier learn [path]` scoped mode — optional path argument to focus on a specific subdirectory (e.g., a monorepo package) instead of the entire repo
- App name derivation from scoped package manifest (`name` field with `@org/` stripping and kebab-case), falling back to leaf directory name
- Scoped output path: notes written to `repos/<monorepo-name>/<app-name>/` instead of `repos/<repo-name>/`
- Scoped-mode substitution block in Step 4 remora prompts — all 3 remoras target `{scope-path}` with root-peek permission for shared config
- Step 7 scoped guard — per-workspace agent spawning is skipped when learn is already scoped to a package
- `/xavier investigate <symptom>` skill — hypothesis-driven bug investigation that spawns parallel remoras across 5 fixed axes (code path, recent changes, dependency boundaries, test coverage, error patterns) plus 1-2 dynamic axes per symptom
- `--file <path>` and `--test <name>` flags for anchoring investigation to a specific entry point; `--file` is canonicalized and required to resolve under the repo root
- Prior investigation check: re-running on the same repo offers to build on existing findings, with a canonical `symptom_summary` field used for matching and display
- `type: investigation` in the Zettelkasten schema with `symptom` and `verdict` as type-specific fields
- `investigations/` vault directory scaffolded by installer, for persisting ranked diagnoses with evidence trails
- `investigations-index` added to the router's requires vocabulary, following the `*-index` pattern — enables skills to declare reads from `investigations/` per the vault-path-declaration rule

### Changed

- Investigate note filename gains an `HHMM` suffix on collision when the user chose "New" (not "Related"), preventing accidental overwrite of unrelated prior investigations

### Deprecated

### Removed

### Fixed

- Review skill leaked "Dispatch scripts for multi-model debate are not installed" message when `agent` CLI was absent — silenced the check and removed "fallback" language that primed the model to narrate the branch
- Review pre-flight now verifies both `agent` CLI and `dispatch.sh`/`parse.sh` exist before enabling the debate path, preventing mid-execution failures when deps are missing
- Self-update could skip distributed deps installation because the replacement was split across three code blocks — merged into a single atomic Bash command

### Security

- Investigate remora prompts wrap user-supplied symptom and prior-investigation content in `<user-symptom>` / `<prior-investigation>` XML blocks with explicit "reference data only" framing, mirroring the research skill's prompt-injection mitigation

## [0.3.0] - 2026-04-15

### Added

- `/xavier research <topic>` skill — topic-first "teach me" command that spawns parallel remoras across web, internal docs, and codebase to produce a structured digest
- `research-index` as 14th vocabulary key in the requires system, following the `*-index` pattern
- `type: research` in the Zettelkasten schema with `topic` and `sources` as type-specific fields
- `research/` vault directory scaffolded by installer for persisting research digests
- `--plan` flag for `/xavier research` to preview and edit decomposed questions before spawning remoras
- Prior research check-and-augment: re-running research on an existing topic offers to update rather than start fresh
- `/xavier feedback` skill to open a GitHub Discussion in the upstream repo, with category selection via GraphQL ([#2](https://github.com/atilafassina/xavier/pull/2))
- `/xavier bug` skill to file a GitHub Issue with structured prompts (skill, expected, actual) and auto-appended Xavier version and OS info ([#2](https://github.com/atilafassina/xavier/pull/2))
- Multi-model review via `multi-model-dispatch` dependency skill with parallel GPT + Gemini reviews, merged findings, and attribution ([#3](https://github.com/atilafassina/xavier/pull/3))
- `debate` reference pattern for structured multi-model dispute resolution ([#3](https://github.com/atilafassina/xavier/pull/3))
- Pilot fish structured brief with vault overlay for reviewer context ([#3](https://github.com/atilafassina/xavier/pull/3))
- Decision log schema for recurring pattern feedback ([#3](https://github.com/atilafassina/xavier/pull/3))

### Changed

- Installer and self-update now distribute dependency skills alongside skills and references ([#4](https://github.com/atilafassina/xavier/pull/4))
- Model labels pass through as GPT/Gemini instead of generic A/B identifiers in parse.sh ([#4](https://github.com/atilafassina/xavier/pull/4))
- Debate output contract relaxed: Description/Scenario fields optional for v1 ([#4](https://github.com/atilafassina/xavier/pull/4))
- Vault-contradicted dispute format added to structured brief template ([#4](https://github.com/atilafassina/xavier/pull/4))
- Alias skills now delegate to xavier router via `Skill` tool instead of bypassing ([#5](https://github.com/atilafassina/xavier/pull/5))
- Self-update skill includes alias regeneration step ([#5](https://github.com/atilafassina/xavier/pull/5))

### Deprecated

### Removed

### Fixed

- `self-update` skill now declares `deps-index:optional` in requires (was reading from `deps/` without declaring it)
- Replace `which agent` with `command -v agent` for POSIX portability in dispatch.sh ([#4](https://github.com/atilafassina/xavier/pull/4))
- Add timeout/gtimeout fallback in dispatch.sh for macOS compatibility ([#4](https://github.com/atilafassina/xavier/pull/4))
- Guard `.prev` file collision during installer upgrades ([#4](https://github.com/atilafassina/xavier/pull/4))
- Clean up broken dependency symlinks during installation ([#4](https://github.com/atilafassina/xavier/pull/4))
- Fix `suggestion` field key in skill frontmatter ([#4](https://github.com/atilafassina/xavier/pull/4))
- Safe symlink replacement for dependency skills ([#4](https://github.com/atilafassina/xavier/pull/4))

### Security

## [0.2.0] - 2026-04-09

### Added

- Cursor IDE runtime support with adapter mapping spawn/collect/poll to Task/Shell/Await tools ([#1](https://github.com/atilafassina/xavier/pull/1))
- Multi-runtime installer that detects and wires all available runtimes simultaneously ([#1](https://github.com/atilafassina/xavier/pull/1))
- Configurable alias prefix with input validation and security hardening ([#1](https://github.com/atilafassina/xavier/pull/1))
- Per-command aliases for Claude Code and Cursor discoverability ([#1](https://github.com/atilafassina/xavier/pull/1))
- Skill validation script (`validate-skills.sh`) ([#1](https://github.com/atilafassina/xavier/pull/1))

### Changed

- Installer refactored to support adapter abstraction across runtimes ([#1](https://github.com/atilafassina/xavier/pull/1))
- Uninstaller updated to handle multi-runtime cleanup ([#1](https://github.com/atilafassina/xavier/pull/1))
- Skill definitions updated to work through the adapter layer ([#1](https://github.com/atilafassina/xavier/pull/1))

### Deprecated

### Removed

### Fixed

### Security

[Unreleased]: https://github.com/atilafassina/xavier/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/atilafassina/xavier/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/atilafassina/xavier/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/atilafassina/xavier/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/atilafassina/xavier/releases/tag/v0.2.0
