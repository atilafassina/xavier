# Backpressure Detection

Detect available backpressure commands by scanning project root for build/test configuration files.

## Detection Table

| Config file | What to check | Suggested commands |
|---|---|---|
| `package.json` | `scripts` keys (`test`, `build`, `lint`, `typecheck`) | `npm test` / `npm run build` / etc. |
| `Cargo.toml` | presence | `cargo test`, `cargo clippy -- -D warnings` |
| `pyproject.toml` | tools in optional-dependencies | `pytest`, `mypy .`, `ruff check .` |
| `go.mod` | presence | `go test ./...`, `go vet ./...` |
| `Makefile` | targets like `test`, `check`, `lint` | `make test` / `make check` |

Only include commands that actually exist in the project. Run each candidate command with a dry-run or version flag to verify availability before including it in the backpressure list.

Skills that detect backpressure commands should reference this table rather than maintaining their own copy.
