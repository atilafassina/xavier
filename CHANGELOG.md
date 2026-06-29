# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`xavier-tool` native binary — a deterministic tool-call layer.** A precompiled Rust binary (workspace in the repo's top-level `tool/`: an `xavier-core` library crate + a thin `xavier-tool` CLI) that moves mechanical work out of LLM-interpreted shell into typed, tested, reproducible code. It ships prebuilt per-triple inside the release tarball, so users compile nothing — the Rust toolchain is a build-time/CI dependency only. Uniform ABI: JSON on stdin, JSON on stdout, status via exit code. `merge` is tool #1; the library/CLI split keeps a future MCP server able to wrap the same code
- `merge` and `merge-text` subcommands — `merge` takes pre-parsed `MergeInput` JSON; `merge-text` takes each model's raw assistant text and parses the findings inside the binary. Both support `--format json|debate-md`
- Content-addressed memoization (`xavier-core::cache`) — `merge`/`merge-text` output is cached on disk keyed on `(subcommand, input, binary version)`, self-invalidating across versions with atomic writes, and replayed byte-for-byte on a repeat. `--no-cache` bypasses it; `XAVIER_TOOL_CACHE_DIR` overrides the location; `XAVIER_TOOL_CACHE_DEBUG=1` logs hit/miss to stderr
- `XAVIER_TOOL_DISABLE` kill switch — any non-empty value forces the `parse.sh` shell fallback even when a healthy binary is installed: an instant operational rollback with no uninstall or file deletion
- Multi-platform release matrix in `.github/workflows/release.yml` — a hand-written GitHub Actions matrix cross-compiles `xavier-tool` for `{x86_64,aarch64} × {apple-darwin, unknown-linux-gnu}` and assembles all four binaries into the single `xavier.tar.gz` (one tarball, N triples)
- Host target-triple detection and selection in `xavier/install.sh` — installs the matching bundled binary and no-ops (no stub) on unsupported platforms, leaving them on the shell merge
- `validate-install-triples.sh` — an offline guard (no network, builds nothing) over the `uname`→triple mapping, the missing-binary fallback, and the `XAVIER_TOOL_DISABLE` kill switch, by eval'ing the live `install.sh` / `merge.sh` functions under a stubbed `uname`
- `.github/workflows/ci.yml` — runs `cargo build` / `test` / `clippy -- -D warnings` / `fmt --check` on Linux and macOS plus the three `validate-*.sh` guards on every pull request and push to `main`

### Changed

- Multi-model review merging now routes through `xavier-tool` via a binary-first front door (`xavier/deps/multi-model-dispatch/merge.sh`) that probes the binary and **gracefully falls back to `parse.sh`** on any problem (missing, incompatible, non-zero exit), so output stays equivalent and a skill never crashes because the binary is absent
- The mechanical merge is the determinism boundary — the binary emits a `## Unmatched` residue bucket and the `review` skill's model pass adjudicates only that bucket; `## Consensus` / `## Disputes` / `## Blindspots` are final and pass through untouched

### Deprecated

### Removed

### Fixed

- Paraphrased near-duplicate findings now collapse into one `## Consensus` entry instead of two separate blindspots. The shell merge matched exact `file:line` only, so "missing field `id`" and "`id` is absent" at one location were recorded as two findings; the binary canonicalizes `file:line(-range)` references and adds textual near-duplicate matching (pure-Rust similarity, threshold-gated), routing genuinely ambiguous pairs to `## Unmatched` instead of guessing
- Robust finding parsing in the binary path — multi-line descriptions, `\uXXXX` escapes (including UTF-16 surrogate pairs), and non-strict markdown (`**File:**`, list bullets, extra spacing) that `parse.sh`'s `awk` scraper mishandled. The shell fallback path is unchanged

### Security

## [0.7.1] - 2026-06-05

### Added

### Changed

- ACE-backed reviews now dispatch GPT reviewers with `gpt-5.5-extra-high` instead of `gpt-5.4-xhigh`, keeping Xavier's multi-model debate pair aligned with the current Cursor `agent` model catalog.

### Deprecated

### Removed

### Fixed

### Security

## [0.7.0] - 2026-06-02

### Added

- **Codex runtime adapter (experimental)** — Shark-pattern support via Codex's built-in `spawn_agent`, `wait_agent`, and `exec_command` tools. `xavier/references/adapters/codex/adapter.md` maps `spawn`/`collect`/`poll` to Codex primitives, with `explorer`/`worker`/`default` agent-type heuristics derived from Xavier task intent (research → explorer, implementation → worker, ambiguous → default). Subagents inherit the parent model by default. When `spawn_agent` is unavailable, Shark flows degrade to inline execution with a one-time warning
- Codex per-command skill aliases — installer generates `~/.agents/skills/${ALIAS_PREFIX}-${cmd}/SKILL.md` stubs that delegate to the vault router, mirroring the Claude Code (`~/.claude/commands/`) and Cursor (`~/.cursor/skills/`) wiring. Codex is auto-detected via `codex` on PATH; `uninstall.sh` removes the aliases as part of the standard runtime sweep
- Remora label discipline for Codex — every `spawn_agent` message is prefixed with `Xavier remora: <label>`, and the adapter requires a `{ label, nickname, handle }` map so user-facing status uses human labels (e.g. "Waiting for 3 remoras: Foundations …; AI deck tools …; Local context") rather than raw `019e…` agent hashes
- Interactive-gate enforcement across runtimes — router (`xavier/SKILL.md`), Codex adapter, installer Codex alias template, and the `grill` / `prd` / `research` / `investigate` / `tasks` skills treat `AskUserQuestion`, confirm, and wait prompts as hard command boundaries via `<stop-guardrail>` blocks. Routed skills never auto-progress from `grill` → `prd` → `tasks` → `loop` without explicit user input
- `PRESERVE_CONFIG=true` refresh path in `xavier/install.sh` — re-running the installer for alias regeneration preserves the existing `config.md` adapter selection instead of overwriting it, and clone-mode reinstall also refreshes the `<XAVIER_HOME>/SKILL.md` router symlink so Codex aliases stay in sync with installed skill logic

### Changed

- `validate-skills.sh` gains adapter and Codex-wiring checks: validates each `adapter.md` has `name`/`type`/`runtime` frontmatter plus `spawn`/`collect`/`poll`/Tool Dispatch sections, and enforces Codex alias generation, vault router refresh, `PRESERVE_CONFIG` path, uninstall sweep, remora label discipline, interactive-gate documentation, `<stop-guardrail>` presence in routed skills, `refresh_available_adapters` existence, and per-runtime detection gating in installer and self-update
- `xavier/install.sh`, `/xavier setup`, and `/xavier self-update` now gate per-runtime symlinks and per-command aliases on detected runtimes — the installer and self-update via `command -v <runtime>` (`case " $DETECTED_RUNTIMES "`), and the `/xavier setup` skill via the runtimes recorded in `available-adapters` from its Step 3. Previously, a Claude-only user would accumulate ~20 stub directories at `~/.cursor/skills/xavier-*/` and `~/.agents/skills/xavier-*/` regardless of whether Cursor or Codex were installed; now those roots stay clean. The `~/.agents/skills/xavier` base symlink and `~/.claude/commands/xavier.md` + `/x.md` symlinks are likewise gated
- Refresh-install path (installer option `[s]`) now extends `available-adapters` in `config.md` when new runtimes are detected — preserving the user's primary `adapter:` choice but updating the advertised list. Previously, `PRESERVE_CONFIG=true` skipped both fields, leaving a Claude-only vault unable to advertise newly-installed Cursor/Codex without rerunning `/xavier setup`. The insertion uses awk (BSD/GNU sed disagree on `\n` in replacements, so the prior `s/.../...\n.../` form silently inserted a literal `\n` on macOS)
- `/xavier self-update` Step 8a now reconciles detected runtimes against `available-adapters`. When `command -v` finds runtimes not yet advertised in `config.md`, the skill prompts via `AskUserQuestion` with three options: extend the list keeping the current primary, extend and switch primary to a newly-detected runtime, or skip the config change entirely. When detection matches the advertised list, no prompt fires and the update remains silent. This closes the gap where users who gained a runtime between installs would have aliases written but `config.md` untouched, and treats the primary-switch decision as a hard interactive gate (no inferred default)
- README runtime table lists Codex as **Experimental** (Claude Code and Cursor remain Full support)

### Deprecated

### Removed

### Fixed

### Security

## [0.6.0] - 2026-05-25

### Added

- `/xavier ask "<question>"` skill — read-first Q&A grounded in the user's captured vault knowledge (`knowledge/repos/{repo}/decisions.md`, `architecture.md`, team conventions via `related:` wikilinks, `recurring-patterns` from recent reviews) with relevance-matched reads of `research/`, `investigations/`, and `knowledge/qa/`. Synthesizes an answer in TL;DR + Evidence + Sources format with inline `[[wikilinks]]` to source notes
- User-confirmed research fallback in `/xavier ask` — when the vault is thin on the topic (floor rule: no salient noun from the question is mentioned in any loaded note; otherwise model judgment), the skill prompts before spawning an adaptive count of remoras (1 narrow / 3 design / 5 exploratory) via `adapter.collect()`. Research remoras are scoped to the current repo (grep, git history, vault deep-scan) — narrower than `/xavier research`'s broad-topic axes
- Asymmetric persistence in `/xavier ask`: research-fallback answers auto-save to `knowledge/qa/{repo}_{YYYY-MM-DD}_{slug}.md` (net-new info); vault-only answers prompt `save? (y/n)` with default No (redundant with source notes). Slug derivation tokenizes the question, filters stop-words, takes the first 5 content words; collisions resolved with deterministic numeric suffix (`-2`, `-3`, …)
- Empty-vault graceful degrade in `/xavier ask`: if `knowledge/repos/{repo}/` doesn't exist, skips vault read and routes straight to the research-fallback prompt with a `💡 Run /xavier learn to cache repo knowledge` tip appended
- `--repo <name>` flag for `/xavier ask` to override cwd-derived repo scope, validated against the `knowledge/repos` segment grammar (`^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$`) before any filesystem read; rejects `/`, `\`, `..`, leading `.`, whitespace, and absolute paths
- Bare invocation `/xavier ask` prompts once via `AskUserQuestion` for the question — single-turn, no follow-up loop. Detect-and-defer: when invoked inside an outer Shark loop (`SHARK_TASK_HASH` set), bare invocation and the save / research prompts are all suppressed so the skill behaves as a non-interactive executor
- `qa-index` vocabulary key in the router — lists `.md` files in `<vault>/knowledge/qa/` with titles and frontmatter (no body), mirroring `research-index` and `investigations-index`. Brings the requires vocabulary to 15 keys
- `type: qa` in the Zettelkasten schema with `question` as the type-specific field — original question text passed to `/xavier ask`, used by `qa-index` cache lookups to surface related prior answers
- Prior Q&A as a cache source — `/xavier ask` reads `qa-index` alongside research/investigations indexes and full-reads matching notes on relevance, citing them via `[[knowledge/qa/<filename>]]` in Evidence / Sources

### Changed

- `validate-xavier-frontmatter.sh` now recognizes `ask` as a note-writing skill and `qa-index` in the allowed `requires:` vocabulary
- `validate-skills.sh` enforces that any skill reading `<vault>/knowledge/qa/` must declare `qa-index` in its `requires:` list (mirrors the existing rule for `research/`)

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

[Unreleased]: https://github.com/atilafassina/xavier/compare/v0.7.1...HEAD
[0.7.1]: https://github.com/atilafassina/xavier/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/atilafassina/xavier/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/atilafassina/xavier/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/atilafassina/xavier/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/atilafassina/xavier/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/atilafassina/xavier/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/atilafassina/xavier/releases/tag/v0.2.0
