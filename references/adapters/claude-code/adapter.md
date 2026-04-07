---
name: claude-code
type: adapter
runtime: claude-code
---

# Claude Code Runtime Adapter

This adapter maps Xavier's runtime operations to Claude Code's built-in tools.

## spawn(task, options) -> handle

Use the `Agent` tool:

```
Agent(
  prompt: task,
  description: options.name (truncated to 3-5 words),
  run_in_background: options.background,
  isolation: options.isolation (if set),
  subagent_type: "general-purpose"
)
```

The **handle** is the agent's ID returned by the Agent tool. Store it with the agent's `name` for later collection.

## poll(handle) -> status

Claude Code automatically notifies when background agents complete. No explicit polling is needed — the runtime delivers a notification with the agent's result when it finishes.

- If the agent has completed: `{ done: true, result: <agent output> }`
- If not yet: `{ done: false }`

## collect(handles[]) -> results[]

To collect results from multiple background agents:

1. Spawn all agents with `run_in_background: true` in a **single message** (parallel tool calls)
2. Wait for completion notifications from each agent
3. As each notification arrives, record `{ name, result }` in the results array
4. Once all handles have reported, return the full results array

## Detection

Xavier detects Claude Code as the active runtime by checking:

1. The `Agent` tool is available in the current session
2. The `Bash` tool is available in the current session

If both are present, Claude Code is the active runtime.

## Limitations

- Background agents cannot use `AskUserQuestion` — they run non-interactively
- Agent results may be large; the pilot fish should summarize rather than pass through raw output
- Worktree isolation requires the current directory to be a git repo
