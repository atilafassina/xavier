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

## Transition Operations

When transitioning a file, perform exactly the actions listed for the target state. Each transition must be **idempotent**: running a transition that matches the current state is a no-op (do not re-edit frontmatter, do not run `mv`).

### `→ done`

1. If the file is already at `<vault>/<kind>/done/<name>.md` AND its frontmatter `status` is already `done`: no-op. Stop.
2. Set the frontmatter `status` field to `done`. If the field is absent, insert it. If it is `superseded`, overwrite it.
3. If the file is at the top level, run `mv <vault>/<kind>/<name>.md <vault>/<kind>/done/<name>.md`.
4. Update the `updated:` ISO date in frontmatter.

### `→ superseded`

1. If the file is already at `<vault>/<kind>/done/<name>.md` AND its frontmatter `status` is already `superseded`: no-op. Stop.
2. Set the frontmatter `status` field to `superseded`. If absent, insert it. If `done`, overwrite it.
3. If the file is at the top level, run `mv <vault>/<kind>/<name>.md <vault>/<kind>/done/<name>.md`.
4. Update the `updated:` ISO date in frontmatter.

### `→ active`

1. If the file is already at the top level (`<vault>/<kind>/<name>.md`) AND has no `status` field: no-op. Stop.
2. Remove the `status` field from frontmatter entirely (do not leave an empty value).
3. If the file is in `done/`, run `mv <vault>/<kind>/done/<name>.md <vault>/<kind>/<name>.md`.
4. Update the `updated:` ISO date in frontmatter.

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

1. Resolve `<name>` against all four candidate paths:
   - `<vault>/prd/<name>.md`
   - `<vault>/prd/done/<name>.md`
   - `<vault>/tasks/<name>.md`
   - `<vault>/tasks/done/<name>.md`
2. **Zero matches** → error: `No PRD or task named '<name>' found.` List the closest matches by basename if any. Stop.
3. **Multiple matches across `<kind>/` and `<kind>/done/`** (i.e., the same `<name>.md` exists at both top level and in `done/` for the same kind, OR exists in both `prd/` and `tasks/` trees) → error:

   ```
   Ambiguous: '<name>' matches multiple files:
     - <vault>/prd/<name>.md
     - <vault>/prd/done/<name>.md
   Disambiguate by re-running with the explicit path or use the picker mode (no args).
   ```

   Stop. Do not perform any transition.
4. **Exactly one match** → dispatch the transition operation for `<state>`.

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

**What it scans.** Glob `<vault>/prd/*.md` (top-level, active PRDs only). For each PRD, find all sibling tasks by:

1. Globbing `<vault>/tasks/*.md` and `<vault>/tasks/done/*.md`.
2. Reading each task's frontmatter `source` field — a wikilink of the form `"[[prd/<prd-name>]]"`.
3. Keeping tasks whose extracted `<prd-name>` matches the PRD under inspection.

A PRD is **eligible** when it has **at least one sibling task** AND **every** sibling is `done` (lives in `<vault>/tasks/done/` OR has frontmatter `status: done` or `status: superseded`). Note: sub-phase 5a will have just moved tasks into `tasks/done/`, so re-glob — do not rely on a snapshot taken before sub-phase 5a ran.

PRDs with no derived tasks are **not** eligible here — they are surfaced in sub-phase 5c instead. Skip PRDs already at `<vault>/prd/done/<name>.md` (idempotent).

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
