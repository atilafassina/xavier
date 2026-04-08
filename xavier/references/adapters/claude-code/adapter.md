---
name: claude-code
type: adapter
runtime: claude-code
---

# Claude Code Runtime Adapter

This adapter maps Xavier's runtime operations to Claude Code's built-in tools.

## spawn(task, options)

Spawn a single background agent.

**Mapping to Claude Code:**

```
Agent(
  prompt: task,
  description: options.name (truncated to 3-5 words),
  run_in_background: options.background ?? true,
  isolation: options.isolation (if set),
  subagent_type: options.subagent_type ?? "general-purpose"
)
```

**Auto-injected behavior:**

- Sets `SHARK_TASK_HASH` in the agent's prompt preamble: `"Export SHARK_TASK_HASH={hash} before starting work."` where `{hash}` is a unique identifier for the current shark flow. This prevents nested shark flows from starting their own orchestration.

The **handle** is the agent's ID returned by the Agent tool.

## collect(tasks[])

Spawn multiple agents concurrently in a **single message** with parallel tool calls. Each task in the array is spawned via `spawn()` with `run_in_background: true`.

**Mapping to Claude Code:**

```
// All spawned in ONE message — parallel background agents
Agent(prompt: tasks[0].task, description: tasks[0].name, run_in_background: true, ...)
Agent(prompt: tasks[1].task, description: tasks[1].name, run_in_background: true, ...)
Agent(prompt: tasks[2].task, description: tasks[2].name, run_in_background: true, ...)
```

**Auto-injected behavior:**

- Sets `SHARK_TASK_HASH` in each agent's prompt preamble (same as `spawn()`)

Results are collected as each agent completes (Claude Code auto-notifies). No polling needed.

## poll(handle)

Claude Code auto-notifies when background agents complete. No explicit polling is needed — the orchestrator receives results automatically.

**Mapping to Claude Code:**

No-op. The Agent tool delivers results on completion.

## Tool Dispatch

Abstract operations mapped to Claude Code tools:

| Operation | Tool |
|-----------|------|
| `run-command` | `Bash` |
| `read-file` | `Read` |
| `write-file` | `Write` |
| `spawn-agent` | `Agent` |
| `poll-agent` | No-op (auto-notifies) |
| `search-text` | `Grep` |
| `search-files` | `Glob` |

## Detection

Xavier detects Claude Code as the active runtime by checking:

1. The `Agent` tool is available in the current session
2. The `Bash` tool is available in the current session

If both are present, Claude Code is the active runtime.

## Limitations

- Background agents cannot use `AskUserQuestion` — they run non-interactively
- Agent results may be large; the pilot fish should summarize rather than pass through raw output
- Worktree isolation requires the current directory to be a git repo

