# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `/xavier learn [path]` scoped mode — optional path argument to focus on a specific subdirectory (e.g., a monorepo package) instead of the entire repo
- App name derivation from scoped package manifest (`name` field with `@org/` stripping and kebab-case), falling back to leaf directory name
- Scoped output path: notes written to `repos/<monorepo-name>/<app-name>/` instead of `repos/<repo-name>/`
- Scoped-mode substitution block in Step 4 remora prompts — all 3 remoras target `{scope-path}` with root-peek permission for shared config
- Step 7 scoped guard — per-workspace agent spawning is skipped when learn is already scoped to a package

### Changed

### Deprecated

### Removed

### Fixed

- Review skill leaked "Dispatch scripts for multi-model debate are not installed" message when `agent` CLI was absent — silenced the check and removed "fallback" language that primed the model to narrate the branch
- Review pre-flight now verifies both `agent` CLI and `dispatch.sh`/`parse.sh` exist before enabling the debate path, preventing mid-execution failures when deps are missing
- Self-update could skip distributed deps installation because the replacement was split across three code blocks — merged into a single atomic Bash command

### Security

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

[Unreleased]: https://github.com/atilafassina/xavier/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/atilafassina/xavier/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/atilafassina/xavier/releases/tag/v0.2.0
