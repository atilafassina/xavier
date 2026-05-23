---
name: loop
description: Execute a task file (or freeform task) as an autonomous loop using the Shark pattern.
requires: [config, shark, tasks-index, prd-index:optional]
---

# Loop

`/xavier loop`

Execute a task file (or freeform task) as an autonomous loop using the Shark pattern. The loop acts as the shark — it delegates each phase to a remora (background agent) and evaluates completion via backpressure commands.

## Step 1: Gather Loop Configuration

1. **Task source**: Accept either:
   - A task file from `~/.xavier/tasks/` — list available files and let the user pick, or accept a **basename** argument (no path-style input).
   - A freeform task description (at least 2 sentences)
2. **Basename validation — applies to both explicit-arg and picker selections.** Whatever path produced the task name (a basename argument, or the user picking a row from the picker), the resolved `<name>` MUST match `^[a-z0-9][a-z0-9-]{0,63}$` (per the Name Validation rules in `xavier/skills/mark/SKILL.md`). Reject `/`, `\`, `..`, leading `.`, whitespace, absolute paths, and anything outside `[a-z0-9-]`. Never accept arbitrary filesystem paths — the loop reads and executes shell commands from the resolved task file's "Backpressure Commands" section, so an unvalidated path would let any reachable file drive command execution.

   **Picker-mode rejection rule.** If a picker selection produces a basename that does NOT match the allowlist (e.g., a legacy task file with underscores or unusual characters in its name), abort the entire loop with: `Cannot run loop on task '<name>': basename does not match the lifecycle allowlist (^[a-z0-9][a-z0-9-]{0,63}$). Rename the task file to a kebab-case basename (and update its 'source' field if needed), then re-run.` Do NOT silently start a loop whose success path Step 5 will later refuse to commit — that produces silent half-completion and leaves the task invisible to `--backfill` recovery.

   After validation succeeds, resolve `<name>` against the four lifecycle cases:

   - **Active-only** (`~/.xavier/tasks/<name>.md` exists, NOT in `tasks/done/`) → use it directly. Proceed to extraction.
   - **Done-only** (`~/.xavier/tasks/done/<name>.md` exists, no top-level counterpart) → read the file's frontmatter `status` (`done` or `superseded`) to compose the revival message. Then choose the recovery-command form based on whether a cross-kind collision exists. If `<vault>/prd/<name>.md` or `<vault>/prd/done/<name>.md` also exists, `mark` arg mode would hit cross-kind ambiguity — suggest picker mode instead. The full message templates:
     - `task <name> is marked done. Revive it with /xavier mark <name> active first, then re-run.` (status: done; no cross-kind PRD exists)
     - `task <name> is marked superseded. Revive it with /xavier mark <name> active first, then re-run.` (status: superseded; no cross-kind PRD exists)
     - `task <name> is marked done. Revive it by running /xavier mark (no args), selecting tasks/<name>, choosing 'active', then re-run.` (status: done; cross-kind PRD also exists)
     - `task <name> is marked superseded. Revive it by running /xavier mark (no args), selecting tasks/<name>, choosing 'active', then re-run.` (status: superseded; cross-kind PRD also exists)
     - If status is missing or invalid, surface the validator-pointer message regardless of cross-kind state: `task <name> lives in tasks/done/ but its status field is missing or invalid. Run 'bash validate-xavier-frontmatter.sh' against your vault to surface the offending file.`

     Exit cleanly. Never load and execute a `done/`-side task file. Archived tasks are explicitly out of scope — their backpressure commands may be stale, point at moved code, or have been intentionally retired.
   - **Ambiguous** (file exists at BOTH `~/.xavier/tasks/<name>.md` and `~/.xavier/tasks/done/<name>.md`) → silently prefer the active top-level task file.
   - **Missing** (file exists at NEITHER path) → fall through to the existing "task not found" error.
3. **If task file**: extract phases, completion criteria, and backpressure commands from the file (same extraction as ralph-loop)
4. **If freeform**: ask the user for completion criteria, backpressure commands, and max iterations. If the user does not specify backpressure commands, auto-detect them using `references/patterns/backpressure-detection.md`.
5. **Max iterations**: default 10. Warn at >25 about cost implications
6. **Pause before phase**: optional phase number to pause at (default: No)

Present extracted/provided values to the user for confirmation before proceeding.

## Step 2: Pre-flight

Run all checks before starting. If any check fails, stop immediately:

1. **Backpressure commands pass**: run every command now. All must exit 0. Pre-existing failures waste iterations
2. **Git state is clean**: `git status` must show no uncommitted changes
3. **Task is written down**: file path exists and is readable, or freeform description is at least 2 sentences
4. **No stale loop state**: check `~/.xavier/loop-state/<task-name>.md` for existing state for this task. If found, ask to resume or start fresh.

   **Pre-upgrade compatibility**: legacy loop-state files (written before the lifecycle feature) may not start with the `# Loop State` heading defined in Step 3. When resuming an existing file, inspect its first non-blank line: if it is not exactly `# Loop State`, **prepend** the heading (followed by a blank line) before continuing. This single repair makes Step 5's `status: complete` insertion deterministic on resumed legacy loops, and is idempotent — files that already start with the heading are untouched.

## Step 3: Initialize State

Create the state file at `~/.xavier/loop-state/<task-name>.md`. Loop-state files have a fixed top-level shape so downstream consumers (Step 5's completion-marker write, `/xavier mark --backfill` sub-phase 5a's heuristic detection) have stable insertion / parsing points:

```
# Loop State

(Optional `status: complete` marker line written by Step 5 success path.)

(Body sections below — content depends on mode.)
```

The first line of the file MUST be the literal heading `# Loop State`. Step 5's auto-mark logic relies on this heading as the insertion anchor for the `status: complete` marker; without it, the marker placement is undefined and backfill cannot detect completed loops deterministically.

Body content:

- **Task-file mode**: track current phase, iteration count, pass/fail history per phase, learnings
- **Freeform mode**: lighter format — iteration count, progress log, learnings
- Loop state files have **no Zettelkasten frontmatter** (they are ephemeral tracking, not knowledge). The `# Loop State` heading is a structural anchor, not Zettelkasten metadata.

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
- **All phases complete**: run Step 5 (auto-mark task as done), then announce success. **Do not delete `~/.xavier/loop-state/<name>.md`** — Step 5 writes a `status: complete` marker into that file as a stable signal for `/xavier mark --backfill` (sub-phase 5a). Deleting the file would erase that signal and reduce backfill to heuristic detection again. "Clean up state" here means clearing transient in-memory tracking only; the on-disk loop-state file persists with its completion marker.
- **Max iterations reached**: announce limit, summarize remaining work. Do **not** auto-mark — the task is incomplete. The loop-state file persists without `status: complete` so a future run can resume.
- **No progress for 2 consecutive iterations**: announce stall, ask user for guidance. Do **not** auto-mark — the task is incomplete. The loop-state file persists without `status: complete`.

## Step 5: Auto-Mark Source Task as Done (success path only)

When — and only when — the loop reaches the **All phases complete** branch of Step 4h, transition the source task file to `done` automatically. Do this silently — no user prompt.

**Skip this step entirely if any of the following hold:**

- **Freeform mode**: there is no source file to mark. Stop.
- **Max iterations reached**: not a success. Skip.
- **User stop / stall**: not a success. Skip.
- **Partial progress only**: not a success. Skip.

**First, validate the task name.** The `<name>` is the basename of the task file selected in Step 1; it must match the canonical Name Validation regex from `xavier/skills/mark/SKILL.md` exactly: `^[a-z0-9][a-z0-9-]{0,63}$` (1–64 characters). Reuse mark's validator verbatim — never relax it here. If validation fails, abort Step 5 before writing the loop-state completion marker; leave the task as-is and let the user run `/xavier mark` manually after the underlying name is sorted out. Never derive filesystem operations from an unvalidated name, and never half-record completion (loop-state marker without the source-task move) for a name the canonical transition will reject.

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

1. Read the just-completed source task's frontmatter `source` field — it is a wikilink of the form `[[prd/<name>]]`. Extract `<name>` and **validate it as a basename** (must match `^[a-z0-9][a-z0-9-]{0,63}$` per the Name Validation rules in `xavier/skills/mark/SKILL.md`). If validation fails, skip this step entirely — never derive filesystem operations from an unvalidated `source` value. Log a warning that the source field looks malformed.
2. **Resolve `<name>` to an actual PRD file** before doing anything else. Check both candidate paths and branch on the combination:
   - `<vault>/prd/<name>.md` exists AND `<vault>/prd/done/<name>.md` exists (drift / ambiguity) → the vault has both an active and an archived copy of the same basename, which is the exact failure mode the lifecycle contract is meant to prevent. **Do not retire the active PRD silently.** Skip the prompt and log a warning: `PRD <name> exists at both <vault>/prd/<name>.md (active) and <vault>/prd/done/<name>.md (archived). Reconcile via /xavier mark (no args) — pick one to keep, then re-run.` Until the user reconciles, the active PRD stays in `prd-index` and the archived copy is left untouched.
   - Only `<vault>/prd/<name>.md` exists → PRD is active. Continue with the sibling scan below.
   - Only `<vault>/prd/done/<name>.md` exists → PRD is already done. **Skip the prompt.** Stop.
   - Neither exists → the `source` field points at a non-existent PRD (typically legacy task notes that recorded `source` as the task's own filename rather than the PRD basename). **Skip Step 6 entirely** — do not run the sibling scan, do not prompt. Log a warning: `Cannot offer PRD retirement: source [[prd/<name>]] does not resolve to any PRD. Update the task's source field if the PRD lives under a different basename.`
3. Find sibling tasks in a **single pass** instead of reading every task file. The match MUST be anchored to the `source:` frontmatter field — a bare `[[prd/<name>]]` anywhere in a task body or in the `related:` list is **not** evidence the task derives from that PRD, and counting it would let unrelated tasks suppress the prompt or trigger a wrong retirement. Use `find -exec grep -l` to avoid ARG_MAX at scale, and grep against an anchored pattern that accepts all YAML quoting styles for the `source` field:

   ```
   find <vault>/tasks -type f -name '*.md' \
     -exec grep -lE '^source:[[:space:]]*['\''"]?\[\[prd/<name>\]\]['\''"]?[[:space:]]*$' {} + 2>/dev/null
   ```

   The single `<vault>/tasks` argument lets `find` recurse into the `done/` subdir naturally, so this one invocation returns both active siblings and archived siblings without relying on `-maxdepth` (which is a non-POSIX extension some BSD `find` implementations may not provide). The pattern is line-anchored (`^…$`), targets the `source:` key, and tolerates optional surrounding single or double quotes around the wikilink. Since `<name>` is already validated as `[a-z0-9-]{1,64}`, the regex is unambiguous and needs no shell-escaping. The grep returns the file paths of all sibling tasks; we never need to parse their frontmatter beyond classifying location.
4. For every matching sibling, classify into one of three buckets — **active**, **done**, or **superseded**:
   - **active**: lives at top-level (`<vault>/tasks/<name>.md`). Top-level + any status field is non-canonical drift; classify as active anyway and log a warning naming the file (the user must reconcile via `/xavier mark` before retirement can fire). No silent inference past drift.
   - **done**: lives in `<vault>/tasks/done/` AND frontmatter `status: done`.
   - **superseded**: lives in `<vault>/tasks/done/` AND frontmatter `status: superseded`.
   - In `done/` with missing/invalid status → log a warning, treat as drift, do **not** infer past it (skip the entire Step 6 for this PRD until reconciled — the validator should already catch this case).
5. **Branch on the result:**
   - **At least one sibling is active** → do not prompt. The PRD is not yet ready to retire. Stop this step silently.
   - **Every sibling is `done` (no superseded, no active)** → prompt via **AskUserQuestion**:

     > All tasks for PRD `<name>` are now done. Mark the PRD?

     Options: `done`, `superseded`, `skip`. Dispatch:

     - **`done`** → apply the `→ done` transition from `xavier/skills/mark/SKILL.md` to `<vault>/prd/<name>.md`.
     - **`superseded`** → apply the `→ superseded` transition from `xavier/skills/mark/SKILL.md` to the same PRD.
     - **`skip`** → leave the PRD untouched.

   - **Mixed: at least one sibling is `superseded` AND no sibling is active** → the PRD is implementation-complete in some sense but the lifecycle has been intentionally split (some tasks were replaced rather than completed). Auto-claiming "all tasks are done" would lose that distinction. Prompt with a different message:

     > Tasks for PRD `<name>` are all archived (some `done`, some `superseded`). Mark the PRD?

     Options: `done`, `superseded`, `skip`. Dispatch as above. The user picks the retirement state appropriate to the mix.

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
