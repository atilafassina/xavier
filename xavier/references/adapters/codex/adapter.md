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
  message: "Xavier remora: {options.name or derived task label}\n\n" + task,
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

The **handle** is the agent ID returned by `spawn_agent`, but raw handles are internal bookkeeping only. Every Codex remora must also have a user-visible label:

1. Prefer `options.name` when provided by the Xavier task.
2. Otherwise derive a short label from the task purpose, such as `correctness review`, `security review`, `foundations research`, or `local context`.
3. Record the returned Codex nickname alongside the label and handle: `{ label, nickname, handle }`.

When reporting spawned or waiting agents to the user, use the label first. If a raw handle must be shown for debugging, show it only after the label, for example `security review (James, 019e...)`.

**Fallback:**

If `spawn_agent` is unavailable in the current Codex session, warn once:

```
Codex subagents unavailable; running inline, so Shark parallelism is disabled.
```

Then run the task inline in the current agent.

## collect(tasks[])

Spawn multiple agents concurrently in a single message with parallel tool calls. Each task in the array is spawned via `spawn()` using the mapped Codex `agent_type`.

Before spawning, ensure every task has a user-visible label using `task.name` or a derived label. After spawning, maintain an agent map:

```
[
  { label: "Foundations of vibecoded decks", nickname: "Ada", handle: "019e..." },
  { label: "AI deck tools head-to-head", nickname: "Grace", handle: "019e..." }
]
```

**Mapping to Codex:**

```
// All spawned in ONE message through parallel tool calls
spawn_agent(message: "Xavier remora: " + tasks[0].name + "\n\n" + tasks[0].task, agent_type: "explorer", ...)
spawn_agent(message: "Xavier remora: " + tasks[1].name + "\n\n" + tasks[1].task, agent_type: "explorer", ...)
spawn_agent(message: "Xavier remora: " + tasks[2].name + "\n\n" + tasks[2].task, agent_type: "worker", ...)
```

Results are collected with `wait_agent`. Codex also sends completion notifications, but `wait_agent` is the explicit synchronization point when the shark needs a result before proceeding.

**User-facing status rule:** never present raw agent hashes as the primary status list. Before any blocking `wait_agent` call, print a concise status line using labels, for example:

```
Waiting for 3 remoras: Foundations of vibecoded decks; AI deck tools head-to-head; Local context.
```

If the Codex UI or tool output displays handles anyway, immediately follow with the label map so the user can interpret them:

```
Agent map: Foundations of vibecoded decks -> Ada; AI deck tools head-to-head -> Grace; Local context -> Linus.
```

If subagents are unavailable, run the tasks inline one at a time and preserve the same visible warning behavior as `spawn()`.

## poll(handle)

Use `wait_agent(handle)` when the next step depends on a remora result. Use longer waits for long-running implementation or research tasks to avoid busy polling. Before polling, resolve every handle through the agent map and announce the remora labels being waited on; do not say only "waiting for 019e...".

## Interactive Gates

Codex executes Xavier router and skill instructions inline, so interactive gates must be treated as hard command boundaries. Whenever a routed skill says `AskUserQuestion`, ask, prompt, confirm, quiz, wait for the user, or get feedback, Codex must ask the user and stop. Do not infer the answer, choose filenames, execute later steps, or invoke another Xavier command until the user replies.

When a skill reaches a terminal handoff, show the suggested next commands as options only. Do not automatically move from `grill` to `prd`, from `prd` to `tasks`, from `tasks` to `loop`, or from any Xavier skill into code edits unless the user's newest message explicitly asks for that command.

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
