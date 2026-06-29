---
name: multi-model-dispatch
description: Dispatch prompts to external LLM models via the Cursor `agent` CLI and parse stream-json output into structured findings for multi-model review.
type: dependency
version: 0.2.0
source: "forked from ace plugin dispatch mechanics"
created: 2026-04-15
updated: 2026-06-26
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

1. If a native `xavier-tool` binary is installed for the host triple (at `${XAVIER_HOME:-~/.xavier}/bin/<triple>/xavier-tool`) and passes the compatibility probe (it runs and supports the `merge-text` subcommand), the **mechanical merge runs in the binary** (the determinism boundary). `merge.sh` uses `parse.sh extract` to get each model's final assistant **text**, JSON-encodes that raw text, and pipes it to `xavier-tool merge-text --format debate-md` on stdin (the runtime adapter's `run-command` op). The binary then does everything mechanical: it **parses the findings out of the Markdown itself** (handling multi-line descriptions, `\uXXXX` escapes, and non-strict formatting that the old `awk` scraper mishandled), canonicalizes `file:line` / `file:line-range` references, runs **exact + textual near-duplicate matching**, and renders the debate Markdown.
2. Otherwise it transparently `exec`s `parse.sh merge`, whose output is byte-for-byte what Xavier produced before the binary existed.

Every failure mode in the binary path (missing binary, incompatible/old version without `merge-text`, non-zero exit, empty output) degrades to the `parse.sh` fallback, so a skill **never crashes** because the binary is absent. The binary path's Markdown is equivalent to the shell path's (same `## Consensus` / `## Disputes` / `## Blindspots` sections, findings, and attribution); it may differ by trailing blank lines and by **adding a `## Unmatched` section** the shell path does not produce. The pilot fish detects debate format by the first three headings, so the extra section is additive and safe.

**Operational kill switch.** Set `XAVIER_TOOL_DISABLE` to any non-empty value to force the `parse.sh` fallback even when a healthy binary is installed. The automatic fallback only triggers when the binary *fails* its probe; this switch is the manual rollback for the case the probe cannot catch — a binary that runs cleanly but emits output you don't trust. It needs no uninstall or file deletion: set it per-session, or export it fleet-wide (shell profile / CI env), and every merge reverts to the shell path until you unset it.

### The four output buckets (binary path)

The binary classifies every finding into one of four buckets, rendered as Markdown sections:

- **`## Consensus`** — both models flagged the same location. Either an exact canonical `file:line` match (a single line and a line range that starts there collapse to the same key), or a textual **near-duplicate** at the same file (paraphrases of the same issue — "missing field `id`" vs "`id` is absent"). This is the bug fix: paraphrased same-issue findings no longer split into two blindspots.
- **`## Disputes`** — always empty from the merge; produced later by the pilot fish via vault overlay.
- **`## Blindspots`** — a located finding only one model flagged, where the other model had no finding in that file at all.
- **`## Unmatched`** — the residue the matcher could not place **mechanically**: a finding with no usable location, or a same-file counterpart that fell **below the similarity threshold** (same place, different words — same issue or two distinct ones?). This is the ONLY bucket a downstream model should adjudicate; the other three are final and pass through untouched.

The similarity metric is a pure-Rust composition of an overlap coefficient over content tokens and a normalized Levenshtein distance (threshold `0.30`, calibrated against realistic reviewer paraphrases). It is fully isolated inside the binary (`xavier-core::similarity`); the shell does no similarity work.

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
- **Exact-match merge (shell fallback only)**: `parse.sh merge` uses exact `file:line` matching to classify consensus vs. blindspots, so findings about the same issue at different lines become two blindspots instead of one consensus. This is a deliberate simplicity trade-off in the **fallback** path. The native binary (`merge.sh`'s preferred path) removes this limitation: it canonicalizes line ranges and does textual near-duplicate matching, collapsing paraphrased same-issue findings into one consensus and surfacing genuinely ambiguous pairs in the `## Unmatched` section instead of guessing.
- **Finding-parse heuristics**: `parse.sh`'s `awk` scraper matches the `### [severity] description` format used by Xavier personas and keeps only the heading line of each finding (multi-line descriptions/suggestions and `\uXXXX` escapes are mishandled). The native binary's parser is more robust — it folds multi-line descriptions, decodes `\uXXXX` (including surrogate pairs), and tolerates non-strict markdown (`**File:**`, list bullets, extra spacing). Models that deviate badly from the format still fall back to raw text via `extract`.

## Models Supported (v1)

| Model | Notes |
|-------|-------|
| `gpt-5.5-extra-high` | OpenAI, high-reasoning tier |
| `gemini-3.1-pro` | Google, strong at broad coverage |

Additional models can be used by passing any valid model identifier to dispatch.sh. The two listed above are the tested and recommended pair for debate-style reviews.
