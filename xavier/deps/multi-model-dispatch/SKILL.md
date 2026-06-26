---
name: multi-model-dispatch
description: Dispatch prompts to external LLM models via the Cursor `agent` CLI and parse stream-json output into structured findings for multi-model review.
type: dependency
version: 0.1.0
source: "forked from ace plugin dispatch mechanics"
created: 2026-04-15
updated: 2026-04-15
tags:
  - dispatch
  - multi-model
  - debate
  - agent-cli
---

# Multi-Model Dispatch

Dispatches prompts to external LLM models via the Cursor `agent` CLI and parses their stream-json output into structured findings. This dependency-skill provides the plumbing for Xavier's multi-model review: send a diff to two models in parallel, collect their raw output, and merge it into a consensus/disputes/blindspots report.

## Interface

### dispatch.sh

Sends a prompt to a single model via the `agent` CLI.

```
dispatch.sh <model> <workspace> <output-file> <system-prompt> <user-prompt>
```

**Arguments:**

| Arg | Description |
|-----|-------------|
| `model` | Model identifier (e.g., `gpt-5.5-extra-high`, `gemini-3.1-pro`) |
| `workspace` | Path to the repo/workspace directory |
| `output-file` | Path where raw stream-json output is written |
| `system-prompt` | Persona and vault context (prepended to the prompt) |
| `user-prompt` | The diff or query content |

**Environment variables:**

| Var | Default | Description |
|-----|---------|-------------|
| `XAVIER_TIMEOUT` | `1800` (30 min) | Max seconds before the agent is killed |
| `XAVIER_AGENT` | auto-detect | Override path to the `agent` binary |

**Exit codes:**

- `0` — success
- `124` — timeout (partial output may exist in the output file)
- non-zero — agent CLI error

### parse.sh

Extracts and merges structured findings from raw stream-json output. Pure Bash — no Python, no jq.

```
# Extract final assistant text from a single model's output
bash parse.sh extract <file>

# Merge findings from two models into debate format
bash parse.sh merge <file_a> <file_b> [label_a] [label_b]
```

**`extract`** reads a stream-json file and prints the final assistant text block.

**`merge`** runs the full pipeline: extract text from both files, parse findings from each, then classify findings into consensus and blindspots using exact `file:line` matching. Output is Markdown following the debate protocol. Optional `label_a` and `label_b` identify the models in output (e.g., `GPT`, `Gemini`). Defaults to `Model A` / `Model B` if omitted.

### merge.sh

Binary-first front door for the merge step. **Prefer this over calling `parse.sh merge` directly.** Same interface, same Markdown output:

```
bash merge.sh <file_a> <file_b> [label_a] [label_b]
```

`merge.sh` decides the engine at runtime:

1. If a native `xavier-tool` binary is installed for the host triple (at `${XAVIER_HOME:-~/.xavier}/bin/<triple>/xavier-tool`) and passes a `--version` compatibility probe, the **mechanical exact-match merge runs in the binary** (the determinism boundary). `merge.sh` extracts findings from each stream-json file into JSON, pipes that to `xavier-tool merge --format debate-md` on stdin (the runtime adapter's `run-command` op), and passes the rendered Markdown through.
2. Otherwise it transparently `exec`s `parse.sh merge`, whose output is byte-for-byte what Xavier produced before the binary existed.

Every failure mode in the binary path (missing binary, incompatible version, non-zero exit, empty output) degrades to the `parse.sh` fallback, so a skill **never crashes** because the binary is absent. The binary path's Markdown is equivalent to the shell path's (same sections, findings, and attribution); it may differ only by trailing blank lines, which the pilot fish ignores (it detects sections by heading).

The native binary is built in CI (build-time Rust toolchain only) and bundled per-triple inside the release tarball; users never compile it. Its source is the Rust workspace in the repo's top-level `tool/` directory. See `xavier/bin/README.md` for the bundled-binary layout.

## Typical Usage

```bash
TMPDIR=$(mktemp -d)

# 1. Dispatch to both models in parallel
./dispatch.sh gpt-5.5-extra-high "$WORKSPACE" "$TMPDIR/gpt.json" "$SYSTEM_PROMPT" "$DIFF" &
./dispatch.sh gemini-3.1-pro "$WORKSPACE" "$TMPDIR/gemini.json" "$SYSTEM_PROMPT" "$DIFF" &
wait

# 2. Merge findings via the binary-first front door (labels identify models).
#    Falls back to `parse.sh merge` automatically when no binary is installed.
bash merge.sh "$TMPDIR/gpt.json" "$TMPDIR/gemini.json" GPT Gemini
```

## Limitations

- **Requires `agent` CLI**: the Cursor `agent` binary must be installed at `~/.local/bin/agent` or on `$PATH`, or specified via `XAVIER_AGENT`. Without it, dispatch.sh exits with an error.
- **Stream-json format**: output parsing assumes the agent CLI's `--output-format stream-json` produces newline-delimited JSON objects with `type: "assistant"` messages containing `content` blocks. Changes to the agent CLI output schema will break parsing.
- **Timeout behavior**: when the agent times out (exit 124), partial output may be present in the output file. parse.sh handles truncated files gracefully (extracts whatever text is available).
- **No retries**: dispatch.sh does not retry on failure. The caller is responsible for retry logic.
- **Exact-match merge**: parse.sh uses exact `file:line` matching to classify consensus vs. blindspots. Findings at different lines about the same issue become two blindspots instead of one consensus. This is a deliberate simplicity trade-off — fuzzy matching adds a Python dependency.
- **Finding-parse heuristics**: parse.sh uses pattern matching on the `### [severity] description` format used by Xavier personas. Models that deviate from this format will produce fewer parsed findings (raw text is still available via `extract`).

## Models Supported (v1)

| Model | Notes |
|-------|-------|
| `gpt-5.5-extra-high` | OpenAI, high-reasoning tier |
| `gemini-3.1-pro` | Google, strong at broad coverage |

Additional models can be used by passing any valid model identifier to dispatch.sh. The two listed above are the tested and recommended pair for debate-style reviews.
