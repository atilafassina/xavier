---
name: loop
requires: [config, shark, tasks-index, prd-index]
---

# Loop

`/xavier loop`

Execute a task file (or freeform task) as an autonomous loop using the Shark pattern. The loop acts as the shark — it delegates each phase to a remora (background agent) and evaluates completion via backpressure commands.

## Step 1: Gather Loop Configuration

1. **Task source**: Accept either:
   - A task file from `~/.xavier/tasks/` — list available files and let the user pick, or accept a path argument
   - A freeform task description (at least 2 sentences)
2. **If task file**: extract phases, completion criteria, and backpressure commands from the file (same extraction as ralph-loop)
3. **If freeform**: ask the user for completion criteria, backpressure commands, and max iterations. If the user does not specify backpressure commands, auto-detect them using `references/patterns/backpressure-detection.md`.
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
- **All phases complete**: run Step 5 (auto-mark task as done), then announce success and clean up state
- **Max iterations reached**: announce limit, summarize remaining work. Do **not** auto-mark — the task is incomplete
- **No progress for 2 consecutive iterations**: announce stall, ask user for guidance. Do **not** auto-mark — the task is incomplete

## Step 5: Auto-Mark Source Task as Done (success path only)

When — and only when — the loop reaches the **All phases complete** branch of Step 4h, transition the source task file to `done` automatically. Do this silently — no user prompt.

**Skip this step entirely if any of the following hold:**

- **Freeform mode**: there is no source file to mark. Stop.
- **Max iterations reached**: not a success. Skip.
- **User stop / stall**: not a success. Skip.
- **Partial progress only**: not a success. Skip.

**First, validate the task name.** The `<name>` is the basename of the task file selected in Step 1; it must already match the Name Validation rules in `xavier/skills/mark/SKILL.md` (`^[a-z0-9][a-z0-9-]*$`). If validation fails, abort Step 5 — leave the task as-is and let the user run `/xavier mark` manually. Never derive filesystem operations from an unvalidated name.

**Second, write a completion marker to the loop-state file.** Append (or update, if already present) the line `status: complete` at the top of `~/.xavier/loop-state/<name>.md`, immediately after the `# Loop State` heading. This is a stable, machine-readable signal consumed by `/xavier mark --backfill` (sub-phase 5a) so future migrations can detect completed loops without relying on heuristic phase-table parsing. The write is a single short line and must precede the source-task move below — if the move fails and the transition rolls back, the loop-state marker can stay, since rolling forward later (e.g., manually marking the task as done) still leaves the vault consistent.

**Third, apply the `→ done` transition** from `xavier/skills/mark/SKILL.md` to the source task file (the `~/.xavier/tasks/<name>.md` originally selected in Step 1). The `mark` skill owns the canonical contract — name validation, idempotency, move-precondition (refuse to overwrite an existing destination), frontmatter-then-`mv` ordering, and rollback on `mv` failure all live there. Do not duplicate any of those rules here.

If the transition aborts (e.g., because the destination already exists), surface the abort message to the user and stop Step 5 — do not proceed to Step 6. The loop's overall success state is unchanged; only the auto-mark didn't land.

**Loop-state file is unaffected by the source-task move.** The state file at `~/.xavier/loop-state/<name>.md` is keyed by **basename only** — the same `<name>` as the source task file. Moving the source from `<vault>/tasks/<name>.md` to `<vault>/tasks/done/<name>.md` does not touch the loop-state file. Loop-state cleanup is handled by the success branch of Step 4h after this step returns.

**Do not commit here.** The auto-mark frontmatter edit and `mv` are filesystem operations only; the router commits vault changes after the skill completes (mirroring the policy in `mark/SKILL.md`).

## Step 6: Offer to Mark Source PRD (success path only)

After Step 5 has marked the task as `done`, check whether the source PRD is now fully implemented and prompt the user to retire it. This step runs **only on the success path** — same gating as Step 5.

**Skip this step entirely if any of the following hold:**

- **Freeform mode**: the loop has no source task file, so there is no `source` field to read and no PRD to mark. Stop.
- **Source frontmatter has no `source` field, or the field is empty**: the task is not linked to a PRD. Stop.
- **PRD already lives at `<vault>/prd/done/<name>.md`**: the PRD has already been retired. Skip the prompt — there is nothing to do.
- All Step 5 skip conditions (max iterations, user-stop, stall, partial-progress) also apply here transitively, since this step only runs after Step 5's success branch.

**Otherwise, scan sibling tasks and decide whether to prompt:**

1. Read the just-completed source task's frontmatter `source` field — it is a wikilink of the form `[[prd/<name>]]`. Extract `<name>` and **validate it as a basename** (must match `^[a-z0-9][a-z0-9-]*$` per the Name Validation rules in `xavier/skills/mark/SKILL.md`). If validation fails, skip this step entirely — never derive filesystem operations from an unvalidated `source` value. Log a warning that the source field looks malformed.
2. Verify the PRD's current location:
   - If `<vault>/prd/done/<name>.md` exists → PRD is already done. **Skip the prompt.** Stop.
   - Otherwise the PRD lives at `<vault>/prd/<name>.md` (active). Continue.
3. Find sibling tasks in a **single pass** instead of reading every task file. Use `grep -l` (or equivalent) to short-circuit on the source pattern:

   ```
   grep -l 'source:[[:space:]]*"\[\[prd/<name>\]\]"' \
     <vault>/tasks/*.md <vault>/tasks/done/*.md 2>/dev/null
   ```

   Since `<name>` is already validated as `[a-z0-9-]+`, no shell-escaping or regex-escaping is required. The grep returns the file paths of all sibling tasks; we never need to parse their frontmatter beyond classifying location/status.
4. For every matching sibling, classify as **done** or **active**:
   - The sibling is **done** if it lives in `<vault>/tasks/done/` (cheap path check) OR its frontmatter `status` is `done` (or `superseded`). Check the path first; only read the file's frontmatter if it lives at top-level.
   - Otherwise it is **active**. Short-circuit as soon as one active sibling is found — there is no need to classify the rest before deciding to suppress the prompt.
5. **Branch on the result:**
   - **At least one sibling is still active** → do not prompt. The PRD is not yet ready to retire. Stop this step silently.
   - **Every sibling is done** → prompt via **AskUserQuestion**:

     > All tasks for PRD `<name>` are now done. Mark the PRD?

     Options: `done`, `superseded`, `skip`.

     Dispatch based on the answer:

     - **`done`** → apply the `→ done` transition from `xavier/skills/mark/SKILL.md` to `<vault>/prd/<name>.md`. Do not duplicate the transition logic — the canonical operation lives in the `mark` skill.
     - **`superseded`** → apply the `→ superseded` transition from `xavier/skills/mark/SKILL.md` to the same PRD.
     - **`skip`** → leave the PRD untouched. No filesystem or frontmatter change.

**Do not commit here.** The PRD frontmatter edit and `mv` are filesystem operations only; the router commits vault changes after the skill completes (mirroring the policy in `mark/SKILL.md` and Step 5 above).

## Rules

Non-negotiable during a xavier loop:

1. **Never skip backpressure** — they are the source of truth, not your judgment
2. **Never claim completion without passing all backpressure commands**
3. **Never repeat a failed approach** — check learnings before each iteration
4. **Always commit working progress** — don't accumulate uncommitted changes
5. **Shark never implements** — all work is delegated to remoras
6. **Ask the user when stuck** — 2 iterations with no progress triggers a stop
7. **Auto-mark only on success** — Step 5 runs only when every phase passed. Max-iterations, user-stop, stall, partial-progress, and freeform mode all skip auto-mark
