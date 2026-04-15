---
name: multi-model-dispatch
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
| `model` | Model identifier (e.g., `gpt-5.4-xhigh`, `gemini-3.1-pro`) |
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

### parse.py

Extracts and merges structured findings from raw stream-json output.

```
# Extract final assistant text from a single model's output
python3 parse.py --extract <file>

# Merge findings from two models into debate format
python3 parse.py --merge <file_a> <file_b>
```

**`--extract`** reads a stream-json file and prints the final assistant text block.

**`--merge`** runs the full pipeline: extract text from both files, parse findings from each, then classify findings into consensus, disputes, and blindspots. Output is Markdown following the debate protocol.

## Typical Usage

```bash
# 1. Dispatch to both models in parallel
./dispatch.sh gpt-5.4-xhigh "$WORKSPACE" /tmp/gpt.json "$SYSTEM_PROMPT" "$DIFF" &
./dispatch.sh gemini-3.1-pro "$WORKSPACE" /tmp/gemini.json "$SYSTEM_PROMPT" "$DIFF" &
wait

# 2. Merge findings
python3 parse.py --merge /tmp/gpt.json /tmp/gemini.json
```

## Limitations

- **Requires `agent` CLI**: the Cursor `agent` binary must be installed at `~/.local/bin/agent` or on `$PATH`, or specified via `XAVIER_AGENT`. Without it, dispatch.sh exits with an error.
- **Stream-json format**: output parsing assumes the agent CLI's `--output-format stream-json` produces newline-delimited JSON objects with `type: "assistant"` messages containing `content` blocks. Changes to the agent CLI output schema will break parsing.
- **Timeout behavior**: when the agent times out (exit 124), partial output may be present in the output file. parse.py handles truncated files gracefully (extracts whatever text is available).
- **No retries**: dispatch.sh does not retry on failure. The caller is responsible for retry logic.
- **Finding-parse heuristics**: parse.py uses pattern matching on the `### [severity] description` format used by Xavier personas. Models that deviate from this format will produce fewer parsed findings (raw text is still available via `--extract`).

## Models Supported (v1)

| Model | Notes |
|-------|-------|
| `gpt-5.4-xhigh` | OpenAI, high-reasoning tier |
| `gemini-3.1-pro` | Google, strong at broad coverage |

Additional models can be used by passing any valid model identifier to dispatch.sh. The two listed above are the tested and recommended pair for debate-style reviews.
