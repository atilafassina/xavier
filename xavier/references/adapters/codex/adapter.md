---
name: codex
type: adapter
runtime: codex
---

# Codex Runtime Adapter

This adapter maps Xavier's runtime operations to Codex's built-in tools.

## spawn(task, options)

Spawn a single background agent when subagents are available.

**Mapping to Codex:**

```
spawn_agent(
  message: task,
  agent_type: map_agent_type(options.subagent_type, task),
  fork_context: options.fork_context ?? false
)
```

**Agent type mapping:**

| Xavier intent or runtime name | Codex agent_type |
|-------------------------------|------------------|
| research, explore, Explore | explorer |
| implementation, worker, edit, fix | worker |
| general-purpose, generalPurpose, general | default |

Prefer `explorer` for read-only research and codebase discovery. Prefer `worker` for implementation and test-fixing tasks. Use `default` for ambiguous general tasks.

Do not set `model` unless the user explicitly asks for a different model or the task clearly requires one. Codex subagents inherit the parent model by default.

**Auto-injected behavior:**

- Sets `SHARK_TASK_HASH` in the agent's prompt preamble: `"Export SHARK_TASK_HASH={hash} before starting work."` where `{hash}` is a unique identifier for the current shark flow. This prevents nested shark flows from starting their own orchestration.

The **handle** is the agent ID returned by `spawn_agent`.

**Fallback:**

If `spawn_agent` is unavailable in the current Codex session, warn once:

```
Codex subagents unavailable; running inline, so Shark parallelism is disabled.
```

Then run the task inline in the current agent.

## collect(tasks[])

Spawn multiple agents concurrently in a single message with parallel tool calls. Each task in the array is spawned via `spawn()` using the mapped Codex `agent_type`.

**Mapping to Codex:**

```
// All spawned in ONE message through parallel tool calls
spawn_agent(message: tasks[0].task, agent_type: "explorer", ...)
spawn_agent(message: tasks[1].task, agent_type: "explorer", ...)
spawn_agent(message: tasks[2].task, agent_type: "worker", ...)
```

Results are collected with `wait_agent`. Codex also sends completion notifications, but `wait_agent` is the explicit synchronization point when the shark needs a result before proceeding.

If subagents are unavailable, run the tasks inline one at a time and preserve the same visible warning behavior as `spawn()`.

## poll(handle)

Use `wait_agent(targets: [handle])` when the next step depends on a remora result. Use longer waits for long-running implementation or research tasks to avoid busy polling.

## Tool Dispatch

| Operation | Tool |
|-----------|------|
| `run-command` | `exec_command` |
| `read-file` | `exec_command` with `sed`, `nl`, or shell file reads |
| `write-file` | `apply_patch` |
| `spawn-agent` | `spawn_agent` |
| `poll-agent` | `wait_agent` |
| `search-text` | `exec_command` with `rg` |
| `search-files` | `exec_command` with `rg --files` |

## Detection

Xavier detects Codex as the active runtime by checking:

1. The `spawn_agent` tool is available in the current session
2. The `exec_command` tool is available in the current session

If both are present, Codex is the active runtime. If `exec_command` is present but subagent tools are unavailable, Codex may still run Xavier inline with the fallback warning.

## Limitations

- Codex subagents cannot prompt the user for input; they run non-interactively.
- Subagents inherit the parent model by default; adapter calls should not override it by default.
- Background agents may complete via notification, but Shark flows should still use `wait_agent` when they need a blocking result.
- If subagents are unavailable, Shark-dependent skills degrade to inline execution without parallelism.
- Worktree isolation requires the current directory to be a git repo.
