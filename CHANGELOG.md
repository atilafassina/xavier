# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `/xavier feedback` skill to open a GitHub Discussion in the upstream repo, with category selection via GraphQL
- `/xavier bug` skill to file a GitHub Issue with structured prompts (skill, expected, actual) and auto-appended Xavier version and OS info

### Changed

### Deprecated

### Removed

### Fixed

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

[Unreleased]: https://github.com/atilafassina/xavier/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/atilafassina/xavier/releases/tag/v0.2.0
