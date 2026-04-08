---
type: contract
scope: runtime-adapter
---

# Runtime Adapter Contract

A runtime adapter tells Xavier how to spawn agents, run background tasks, and collect results in a specific AI agent runtime. Each adapter must implement the three operations below.

## Operations

### 1. spawn(task, options) -> handle

Spawn a new agent to work on a task.

**Inputs:**
- `task` (string) — the prompt/instructions for the agent
- `options.background` (boolean) — whether to run in background (default: true)
- `options.name` (string) — identifier for the agent (e.g., "reviewer-correctness")
- `options.isolation` (string, optional) — isolation mode (e.g., "worktree")

**Output:**
- `handle` — an opaque identifier the adapter uses to track the spawned agent

### 2. poll(handle) -> status

Check whether a spawned agent has completed.

**Inputs:**
- `handle` — the handle returned by `spawn`

**Output:**
- `status.done` (boolean)
- `status.result` (string, optional) — the agent's output if done

### 3. collect(handles[]) -> results[]

Wait for multiple agents and collect all their results.

**Inputs:**
- `handles[]` — array of handles from `spawn`

**Output:**
- `results[]` — array of `{ name, result, error? }` objects, one per handle

## Adapter File Structure

Each adapter lives in `~/.xavier/adapters/<runtime-name>/` with:

```
adapter.md    — instructions for Xavier on how to use this runtime's tools
README.md     — human-readable description of the adapter
```

The `adapter.md` is what Xavier reads at runtime to know how to call spawn/poll/collect using the tools available in that runtime.
