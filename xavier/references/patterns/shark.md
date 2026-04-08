# Shark Orchestration Protocol

The Shark pattern is an orchestration model where a central coordinator (the shark) delegates all implementation work to autonomous sub-agents (remoras) and evaluates results through backpressure commands.

## Core Principles

1. **Shark delegates, never implements.** The shark's role is to plan, delegate, evaluate, and decide. It never writes code, edits files, or performs implementation work directly.

2. **Backpressure is truth.** The only reliable signal for progress is the output of backpressure commands (tests, linters, type checks). Self-reported success from a remora is not sufficient — the shark must verify with commands.

3. **Remoras are disposable.** Each remora is a single-purpose agent spawned for a specific task. If a remora fails, the shark spawns a new one with corrected context rather than retrying the same agent.

## Remora Spawning Rules

- Spawn remoras via the adapter's `spawn()` function
- When spawning multiple independent remoras, use the adapter's `collect()` function to spawn them all concurrently
- Each remora receives: task description, acceptance criteria, relevant context, and learnings from prior failures
- Remoras do NOT spawn other remoras — only the shark spawns agents
- Remoras report what they did, not whether they succeeded — the shark evaluates via backpressure

## Detect-and-Defer

Before starting a Shark flow, check the environment variable `SHARK_TASK_HASH`:

```bash
echo "$SHARK_TASK_HASH"
```

- **If set** (non-empty): this agent is running inside an outer Shark loop. Do NOT start a new Shark flow. Act as a simple executor — do the work inline and return results to the caller.
- **If unset** (empty): this agent is the top-level orchestrator. Proceed with the full Shark flow.

## Evaluation Loop

After each remora completes:

1. Read remora output
2. Run all backpressure commands
3. If pass → mark task done, advance to next
4. If fail → capture error output, spawn new remora with error context and learnings
5. If 2 consecutive failures with no progress → stop and escalate to user

## State Tracking

The shark maintains a state file tracking:
- Current phase/task
- Iteration count
- Pass/fail history
- Learnings (errors encountered, patterns discovered, approaches to avoid)

The state file must stay under 100 lines to avoid context bloat.
