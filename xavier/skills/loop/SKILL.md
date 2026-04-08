---
name: loop
requires: [config, shark]
---

# Loop

`/xavier loop`

Execute a task file (or freeform task) as an autonomous loop using the Shark pattern. The loop acts as the shark — it delegates each phase to a remora (background agent) and evaluates completion via backpressure commands.

## Step 1: Gather Loop Configuration

1. **Task source**: Accept either:
   - A task file from `~/.xavier/tasks/` — list available files and let the user pick, or accept a path argument
   - A freeform task description (at least 2 sentences)
2. **If task file**: extract phases, completion criteria, and backpressure commands from the file (same extraction as ralph-loop)
3. **If freeform**: ask the user for completion criteria, backpressure commands, and max iterations
4. **Max iterations**: default 10. Warn at >25 about cost implications
5. **Pause before phase**: optional phase number to pause at (default: No)

Present extracted/provided values to the user for confirmation before proceeding.

## Step 2: Pre-flight

Run all checks before starting. If any check fails, stop immediately:

1. **Backpressure commands pass**: run every command now. All must exit 0. Pre-existing failures waste iterations
2. **Git state is clean**: `git status` must show no uncommitted changes
3. **Task is written down**: file path exists and is readable, or freeform description is at least 2 sentences
4. **No stale loop state**: check `~/.xavier/loop-state/` for existing state for this task. If found, ask to resume or start fresh

## Step 3: Initialize State

Create state file at `~/.xavier/loop-state/<task-name>.md`:

- **Task-file mode**: track current phase, iteration count, pass/fail history per phase, learnings
- **Freeform mode**: lighter format — iteration count, progress log, learnings
- Loop state files have **no Zettelkasten frontmatter** (they are ephemeral tracking, not knowledge)

## Step 4: Run the Loop

For each iteration, follow the Shark protocol from the resolved `shark` context:

### 4a. Read State
Read `~/.xavier/loop-state/<task-name>.md` to understand current phase, prior failures, and learnings.

### 4b. Check Phase Pause
If pause-before-phase is set and the current iteration enters that phase, stop and ask the user to confirm.

### 4c. Spawn Remora
The shark identifies the current phase's work, then spawns a **single remora** (background agent) to execute it:

```
spawn(
  task: "You are executing Phase {N} of a task plan.\n## Task\n{phase description}\n## Acceptance Criteria\n{criteria}\n## Learnings from Prior Iterations\n{learnings}\n\nMake the changes, then report what you did.",
  options: { name: "xavier loop phase {N}", background: true }
)
```

The shark does NOT do implementation work itself — it only delegates, evaluates, and decides.

### 4d. Evaluate Remora Output
When the remora completes, read its output. Check if it reports success or failure.

### 4e. Run Backpressure Commands
Run every backpressure command from the state file. Record pass/fail results.

### 4f. Commit Checkpoint
If backpressure passes (or partial progress was made):

```bash
git add -u && git add <new-files> && git commit -m "xavier loop: iteration {N} — {short description}"
```

Never use `git add -A` or `git add .`. Never stage secrets or build artifacts.

### 4g. Update State
Update `~/.xavier/loop-state/<task-name>.md`: increment iteration, log progress, add learnings, update remaining work.

### 4h. Decide Next Action

- **Backpressure passed**: mark phase complete, advance to next phase
- **Backpressure failed**: re-spawn the remora with error context from the failure. Include the exact error output in the prompt so the remora can fix it
- **All phases complete**: announce success, clean up state
- **Max iterations reached**: announce limit, summarize remaining work
- **No progress for 2 consecutive iterations**: announce stall, ask user for guidance

## Rules

Non-negotiable during a xavier loop:

1. **Never skip backpressure** — they are the source of truth, not your judgment
2. **Never claim completion without passing all backpressure commands**
3. **Never repeat a failed approach** — check learnings before each iteration
4. **Always commit working progress** — don't accumulate uncommitted changes
5. **Shark never implements** — all work is delegated to remoras
6. **Ask the user when stuck** — 2 iterations with no progress triggers a stop
