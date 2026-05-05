---
name: mark
requires: [config, prd-index, tasks-index]
---

# Mark

`/xavier mark [name] [state]`

Manually move PRDs and task files between lifecycle states (`active`, `done`, `superseded`). Pairs with the choice-list count hint surfaced by the router — `mark` is what users run to revive an archived item or retire an active one.

## Lifecycle States

There are three states for a PRD or task file:

- **active** — file lives at top level (`<vault>/prd/<name>.md` or `<vault>/tasks/<name>.md`); frontmatter has **no** `status` field. Indexes (`prd-index`, `tasks-index`) only ever surface active items.
- **done** — file lives at `<vault>/<kind>/done/<name>.md`; frontmatter has `status: done`.
- **superseded** — file lives at `<vault>/<kind>/done/<name>.md`; frontmatter has `status: superseded`. Conceptually distinct from `done` (replaced rather than completed) but stored alongside done items.

`<kind>` is `prd` or `tasks`. Knowledge notes, research, investigations, and dependency notes are out of scope for this skill — `mark` only operates on PRDs and tasks.

## Name Validation

Before performing **any** filesystem operation, the `<name>` argument MUST be validated as a basename. Reject anything that is not a basename:

- The name **must** match the regex `^[a-z0-9][a-z0-9-]{0,63}$` (lowercase letters, digits, hyphens; must start with a letter or digit; total length 1–64 characters to keep resulting filenames well below the 255-byte filesystem limit).
- Reject names containing `/`, `\`, `..`, leading `.`, whitespace, absolute paths, or any character outside `[a-z0-9-]`.
- Names sourced from frontmatter wikilinks (e.g., a `source: "[[prd/<name>]]"` field) are **not** trusted — apply the same validation after extracting `<name>` from the wikilink.

If `<name>` fails validation, abort with: `Invalid name '<name>': must match [a-z0-9][a-z0-9-]{0,63}. Aborting — no filesystem changes made.` This rule applies in arg mode (Step 3), one-arg picker pre-filter (Step 2), backfill (Step 5), and any sibling-scan that consumes a `source` field.

## Transition Operations

When transitioning a file, perform exactly the actions listed for the target state, in the order given. Each transition is **idempotent**: running a transition whose end state matches the current state is a no-op (no frontmatter edit, no `mv`). Each transition is **atomic**: if any step fails, prior steps are rolled back so on-disk state remains as it was before the transition started.

The contract below is **canonical** — `loop/SKILL.md` and `tasks/SKILL.md` reference these operations rather than restating them. Do not duplicate transition logic in other skills.

### `→ done`

1. **Validate name** per the rules above. Abort on failure.
2. **Idempotency check.** If the file is already at `<vault>/<kind>/done/<name>.md` AND its frontmatter `status` is already `done`: no-op. Stop.
3. **Move precondition.** If the file currently lives at `<vault>/<kind>/<name>.md` (top-level), verify that `<vault>/<kind>/done/<name>.md` does **not** already exist. If it does, abort with: `Cannot transition <kind>/<name> → done: destination <vault>/<kind>/done/<name>.md already exists. Resolve the conflict manually and re-run.` Do **not** overwrite — `mv` would silently destroy the existing done-side file.
4. **Frontmatter write (first).** At the file's current path, set the frontmatter `status` field to `done` (insert if absent; overwrite if `superseded`). Bump `updated:` to today's ISO date. Save the **prior** `status` value (which may be absent or `superseded`) and the prior `updated:` value in memory in case rollback is needed.
5. **Move (second).** If the file was at the top level, run `mv <vault>/<kind>/<name>.md <vault>/<kind>/done/<name>.md`.
6. **Rollback on move failure.** If `mv` returns non-zero, revert the frontmatter changes from step 4: restore the prior `status` value (re-insert `superseded` if that was the prior value, or remove the `status` field entirely if it was previously absent) and restore the prior `updated:` value. The file ends in exactly the on-disk state it had before the transition started. Surface the original `mv` error to the user.

### `→ superseded`

1. **Validate name** per the rules above. Abort on failure.
2. **Idempotency check.** If the file is already at `<vault>/<kind>/done/<name>.md` AND its frontmatter `status` is already `superseded`: no-op. Stop.
3. **Move precondition.** If the file lives at `<vault>/<kind>/<name>.md` (top-level), verify `<vault>/<kind>/done/<name>.md` does not exist. If it does, abort with the equivalent error from `→ done` step 3. Do not overwrite.
4. **Frontmatter write.** Set `status: superseded` (insert if absent; overwrite if `done`). Bump `updated:`. Save the **prior** `status` value (absent or `done`) and prior `updated:`.
5. **Move.** If at top-level, `mv` to `<vault>/<kind>/done/<name>.md`.
6. **Rollback on move failure.** Restore the prior `status` value (re-insert `done` if that was the prior value, or remove the field if previously absent) and prior `updated:`. The file ends exactly as it was before the transition started.

### `→ active`

1. **Validate name** per the rules above. Abort on failure.
2. **Idempotency check.** If the file is already at `<vault>/<kind>/<name>.md` (top-level) AND has no `status` field: no-op. Stop.
3. **Move precondition.** If the file lives in `<vault>/<kind>/done/<name>.md`, verify `<vault>/<kind>/<name>.md` (top-level) does not already exist. If it does, abort with: `Cannot transition <kind>/<name> → active: destination <vault>/<kind>/<name>.md already exists. Resolve the conflict manually and re-run.` Do not overwrite.
4. **Frontmatter write.** Remove the `status` field entirely (do not leave an empty value). Bump `updated:`. Save prior state.
5. **Move.** If in `done/`, `mv` to `<vault>/<kind>/<name>.md`.
6. **Rollback on move failure.** Re-insert the prior `status` value and restore the prior `updated:` so the file remains in `done/` with its original status.

## Step 1: Parse Arguments

Three invocation modes:

- **No args** → picker mode (Step 2)
- **Two args (`<name> <state>`)** → arg mode (Step 3). `<state>` must be one of `done`, `superseded`, `active`.
- **One arg** → ambiguous; treat as picker mode but pre-filter the list to entries matching the single argument as a name. If exactly one match, prompt for the new state. If no match, surface an error listing what was searched.
- **`--backfill` flag (alone)** → backfill mode (Step 5). One-shot migration that walks loop-state evidence, sibling inference, and a manual sweep to retire pre-existing items in a vault that predates the lifecycle feature.
- **`<state>` not in `{done, superseded, active}`** → error: `Invalid state '<state>'. Allowed: done, superseded, active.` Stop.
- **`--backfill` combined with any other arg** → error: `--backfill must be the only argument.` Stop.

## Step 2: Picker Mode

1. From the resolved `prd-index` and `tasks-index` contexts, gather the active items (top-level `*.md` only — these contexts already exclude `done/`).
2. Glob `<vault>/prd/done/*.md` and `<vault>/tasks/done/*.md` directly to gather the archived items. The indexes do not surface these — read filenames only, no need to read full bodies for the picker.
3. Present the choices using **AskUserQuestion** with `multiSelect: true`. Format the options into two clearly separated sections, **active first**:

   ```
   Active
     1. prd/<name>           (active)
     2. tasks/<name>         (active)
     ...
   Done / Superseded
     N. prd/done/<name>      (done)
     N+1. tasks/done/<name>  (superseded)
     ...
   ```

   Show the `<kind>/<name>` form so the user can disambiguate items with the same base name. Append the current state in parentheses for each row. If a section is empty, render the section header with `(none)` rather than skipping it.

4. After the user selects one or more items, prompt with **AskUserQuestion** for the target state. Options: `done`, `superseded`, `active`.
5. For each selected item, dispatch the transition operation from the matching state above. Apply transitions sequentially.

## Step 3: Arg Mode

1. **Validate `<name>`** per the Name Validation rules above. Abort if it fails.
2. Resolve `<name>` against all four candidate paths:
   - `<vault>/prd/<name>.md`
   - `<vault>/prd/done/<name>.md`
   - `<vault>/tasks/<name>.md`
   - `<vault>/tasks/done/<name>.md`
3. **Zero matches** → error: `No PRD or task named '<name>' found.` List the closest matches by basename if any. Stop.
4. **Multiple matches across `<kind>/` and `<kind>/done/`** (i.e., the same `<name>.md` exists at both top level and in `done/` for the same kind, OR exists in both `prd/` and `tasks/` trees) → error:

   ```
   Ambiguous: '<name>' matches multiple files:
     - <vault>/prd/<name>.md
     - <vault>/prd/done/<name>.md
   Use picker mode (no args) to pick one explicitly.
   ```

   Stop. Do not perform any transition. **Never accept a path-style argument as a workaround** — `<name>` is always a basename. If the user truly has duplicates, they must resolve via the picker.
5. **Exactly one match** → dispatch the transition operation for `<state>`.

## Step 4: Report

After transitions complete, summarize for the user:

- For each item processed, print: `<kind>/<name>: <prior-state> → <new-state>` (or `<kind>/<name>: no change (already <state>)` for no-ops).
- If any transition failed (e.g., write error), surface the error per-item and continue with the remaining items.

## Step 5: Backfill Mode (`--backfill`)

Backfill is a one-shot migration meant to retire items in a vault that predates the lifecycle feature. It runs **three independent sub-phases in sequence**. Each sub-phase is **independently abortable**: answering `no` at one sub-phase's confirm prompt proceeds to the next sub-phase with the vault unchanged so far. Re-running `--backfill` is **idempotent** — items already at `<vault>/<kind>/done/` (or already carrying `status: done` / `status: superseded`) are skipped, so a second run does nothing additional.

Begin by announcing to the user that backfill is a three-step migration and that each step is individually skippable. Then run sub-phases 1 → 2 → 3.

### Sub-phase 5a: auto-batch tasks from loop-state evidence

**What it scans.** Glob `~/.xavier/loop-state/*.md`. For each loop-state file, look for any of the following completion signals (a permissive heuristic — older loop-state files predate a structured marker):

- A line matching `^##\s*Status:\s*COMPLETE` (case-insensitive)
- A line matching `^##\s*Current Phase:\s*COMPLETE`
- A frontmatter-style or list-style `Status:` field whose value is `complete` or `done`
- The literal string `Loop complete` anywhere in the file
- A line `status: complete` (the marker written by the loop skill's Step 5 success path going forward)

If **any** of those patterns matches, the loop-state's basename is a candidate. For each candidate basename `<name>`, look up `<vault>/tasks/<name>.md`:

- If it exists at top-level → eligible for the batch.
- If it already lives in `<vault>/tasks/done/<name>.md` → skip (idempotent).
- If neither path exists → skip (orphan loop-state, not actionable).

**What prompt it shows.** Present the eligible basenames as a single bulk-confirm via **AskUserQuestion**:

> Found N tasks with completed loop-state evidence: `<name1>`, `<name2>`, ... Mark them all as `done`?
>
> Options: `yes`, `no`.

Show the list in full (one per line). Do not paginate.

**Consequence of "no".** The vault is unchanged. **Proceed to sub-phase 5b** — do not abort the entire backfill.

**Consequence of "yes".** For each eligible task, dispatch the `→ done` transition documented above. Apply transitions sequentially; surface per-item failures and continue with remaining items (same contract as Step 4 reporting).

### Sub-phase 5b: PRD inference from sibling tasks

**Runs after sub-phase 5a regardless of its outcome.**

**What it scans.** First, build a `source → {total, done}` map in a **single pass** over `<vault>/tasks/*.md` and `<vault>/tasks/done/*.md` so the eligibility check below runs in O(P) instead of O(P × T):

1. For each task file, read its frontmatter `source` field — a wikilink of the form `"[[prd/<prd-name>]]"` (any quoting style — single-quoted, unquoted, or double-quoted).
2. Extract `<prd-name>` and **validate it** as a basename (must match `^[a-z0-9][a-z0-9-]{0,63}$` per the Name Validation rules). Skip the task and log a warning if validation fails — never derive filesystem operations from an unvalidated `source` value.
3. **Resolve `<prd-name>` to an actual PRD file.** Verify that **at least one** of `<vault>/prd/<prd-name>.md` or `<vault>/prd/done/<prd-name>.md` exists. If neither exists, the task references a non-existent PRD — typically because legacy task notes (predating the lifecycle feature) recorded `source` as the task's own filename rather than the PRD basename. Skip the task entirely and log a warning naming the file and the unresolvable `source` value: `Skipping <task-file>: source [[prd/<prd-name>]] does not resolve to any PRD. Update the task's source field to point at an existing PRD.` Do not include this task in the map; do not propose any PRD retirement based on it.
4. Increment `map[<prd-name>].total`. **Classify the task as done iff it lives in `<vault>/tasks/done/`** — the canonical signal is location, not the frontmatter status. If a task lives at top-level (`<vault>/tasks/<name>.md`) but its frontmatter `status` is `done` or `superseded`, the vault is in non-canonical state (a prior transition's `mv` did not land or a manual edit drifted from the contract). Log a warning naming that file and treat it as **active** for inference purposes — do **not** silently count it as done. The user should run `/xavier mark <name> active` to clear the spurious status, or move the file to `done/` manually.

Note: sub-phase 5a will have just moved tasks into `tasks/done/`, so re-glob — do not rely on a snapshot taken before sub-phase 5a ran.

Then, evaluate eligibility for each PRD via a single map lookup:

- Glob `<vault>/prd/*.md` (top-level, active PRDs only). For each PRD basename `<name>`, look up `map[<name>]`.
- A PRD is **eligible** when `map[<name>].total >= 1` AND `map[<name>].total == map[<name>].done`.
- PRDs with no derived tasks (`map[<name>]` is missing or `total == 0`) are **not** eligible here — they are surfaced in sub-phase 5c instead.
- Skip PRDs already at `<vault>/prd/done/<name>.md` (idempotent).

Cache the map for sub-phase 5c if it needs to know which active items lack derived tasks — do not re-scan.

**What prompt it shows.** Bulk-confirm via **AskUserQuestion**:

> Found N PRDs whose every derived task is now done: `<name1>`, `<name2>`, ... Mark them all as `done`?
>
> Options: `yes`, `no`.

**Consequence of "no".** The vault is unchanged. **Proceed to sub-phase 5c** — do not abort.

**Consequence of "yes".** For each eligible PRD, dispatch the `→ done` transition above.

### Sub-phase 5c: manual sweep

**Runs after sub-phase 5b regardless of its outcome.** This sub-phase is **skippable**.

**What it scans.** All remaining items still active after sub-phases 5a and 5b:

- All top-level files in `<vault>/prd/*.md` (excluding any retired this run)
- All top-level files in `<vault>/tasks/*.md` (excluding any retired this run)

**What prompt it shows.**

1. Present a multi-select picker via **AskUserQuestion** (`multiSelect: true`) listing every remaining item with metadata:
   - `<kind>/<name>`
   - `last-updated:<value>` from the frontmatter `updated:` field (or `?` if absent)
   - `has-derived-tasks:<true|false>` — for PRDs only, `true` iff any task file references this PRD via `source: "[[prd/<name>]]"`. For tasks, omit this column or print `n/a`.
   - `draft:<true|false>` — `true` iff the frontmatter `tags:` array contains `draft`.

   Sort by `last-updated` ascending (oldest first) so stale items rise to the top.

2. After the user selects items (zero or more), prompt for the target state per **the entire selection**: `done`, `superseded`, `skip`. (`skip` exits sub-phase 5c without touching the selection.)

   If users want different states for different items they should run `--backfill` again or use the regular picker mode after backfill exits — `--backfill` deliberately offers a single bulk action per sub-phase to keep the migration short.

**Consequence of "no" / empty selection / `skip`.** Sub-phase 5c exits without touching the vault. Backfill ends.

**Consequence of `done` or `superseded`.** Dispatch the corresponding transition for each selected item.

### Step 6: Backfill Report

After all three sub-phases run, print a single consolidated summary:

```
Backfill summary:
  Sub-phase 5a: <N> tasks marked done (<list>) | <skipped reason>
  Sub-phase 5b: <N> PRDs marked done   (<list>) | <skipped reason>
  Sub-phase 5c: <N> items marked <state> (<list>) | <skipped>
Total moves: <N>. Re-running --backfill is a no-op until new evidence appears.
```

If a sub-phase was answered `no` or selected nothing, record `skipped (user declined)` or `skipped (no candidates)` accordingly.

## Notes

- Do NOT read or modify `done/` files for any kind other than `prd` and `tasks`.
- Do NOT touch the `created:` field — only `updated:` is bumped.
- Do NOT commit changes — the router handles the vault commit after the skill completes.
- The `prd-index` and `tasks-index` contexts already glob top-level only; archived items are reached by direct filesystem globs in the picker.
- Backfill is intended for one-shot migration. After the migration lands, fresh loops auto-mark via `xavier/skills/loop/SKILL.md` Step 5 and PRD prompts in `xavier/skills/loop/SKILL.md` Step 6 / `xavier/skills/tasks/SKILL.md`, so manual `--backfill` use should be rare.
