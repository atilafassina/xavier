# Monorepo Detection

Detect whether the current repository is a monorepo by checking for workspace configuration files.

## Detection Table

| Ecosystem | Config file | Field / marker | How to resolve packages |
|-----------|------------|----------------|------------------------|
| Node (npm/yarn) | `package.json` | `workspaces` array | Resolve glob patterns to directories |
| Node (pnpm) | `pnpm-workspace.yaml` | `packages` array | Resolve glob patterns to directories |
| Rust | `Cargo.toml` | `[workspace]` section with `members` | Resolve glob patterns to directories |
| Go | `go.work` | `use` directives | Listed directories |
| Python | `pyproject.toml` | `[tool.uv.workspace]` or similar | Tool-specific resolution |

## Usage

1. Check files in order from top to bottom. Stop at the first match.
2. If no match, the repo is a single-package project.
3. When monorepo is detected, resolve workspace packages and make the list available for downstream steps.

Skills that need monorepo awareness should reference this pattern rather than inlining their own detection logic.
