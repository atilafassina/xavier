---
name: cursor
type: adapter
runtime: cursor
---

# Cursor Runtime Adapter

This adapter maps Xavier's runtime operations to Cursor's built-in tools.

## spawn(task, options)

Spawn a single background agent.

**Mapping to Cursor:**

```
Task(
  prompt: task,
  description: options.name (truncated to 3-5 words),
  subagent_type: options.subagent_type ?? "generalPurpose",
  run_in_background: options.background ?? true
)
```

**Auto-injected behavior:**
- Sets `SHARK_TASK_HASH` in the agent's prompt preamble: `"Export SHARK_TASK_HASH={hash} before starting work."` where `{hash}` is a unique identifier for the current shark flow. This prevents nested shark flows from starting their own orchestration.

The **handle** is the task ID returned by the Task tool. When `run_in_background` is true, an `output_file` path is also returned for polling.

## poll(handle)

Cursor background tasks do not auto-notify on completion. The shark must poll explicitly.

**Mapping to Cursor:**

```
Await(task_id: handle, block_until_ms: interval)
```

Alternatively, read the `output_file` returned by spawn and check for an `exit_code` footer line to determine completion.

**Polling strategy:** Use incremental backoff starting at 2 seconds, doubling each attempt (2s, 4s, 8s, 16s...) up to a maximum interval of 60 seconds. Reset backoff after receiving new output.

## collect(tasks[])

Spawn multiple agents concurrently in a **single message** with parallel tool calls. Each task in the array is spawned via `spawn()` with `run_in_background: true`.

**Mapping to Cursor:**

```
// All spawned in ONE message — parallel background tasks
Task(prompt: tasks[0].task, description: tasks[0].name, subagent_type: "generalPurpose", run_in_background: true, ...)
Task(prompt: tasks[1].task, description: tasks[1].name, subagent_type: "generalPurpose", run_in_background: true, ...)
Task(prompt: tasks[2].task, description: tasks[2].name, subagent_type: "generalPurpose", run_in_background: true, ...)
```

**Auto-injected behavior:**
- Sets `SHARK_TASK_HASH` in each agent's prompt preamble (same as `spawn()`)

Results are collected by polling each task's handle via `poll()`. All tasks must be polled to completion before proceeding.

## Tool Dispatch

Abstract operations mapped to Cursor tools:

| Operation | Tool |
|-----------|------|
| `run-command` | `Shell` |
| `read-file` | `Read` |
| `write-file` | `Write` |
| `spawn-agent` | `Task` |
| `poll-agent` | `Await` |
| `search-text` | `Grep` |
| `search-files` | `Glob` |

## Detection

Xavier detects Cursor as the active runtime by checking:
1. The `Task` tool is available in the current session
2. The `Shell` tool is available in the current session

If both are present, Cursor is the active runtime.

## Limitations

- Background tasks return results via output files that must be read — no auto-notification
- Task subagents do not have access to the user's message or prior assistant context — all necessary context must be passed in the prompt
- Background tasks cannot prompt the user for input — they run non-interactively
- Task results may be large; the pilot fish should summarize rather than pass through raw output
- Worktree isolation requires the current directory to be a git repo
