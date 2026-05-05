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

Two invocation modes:

- **No args** → picker mode (Step 2)
- **Two args (`<name> <state>`)** → arg mode (Step 3). `<state>` must be one of `done`, `superseded`, `active`.
- **One arg** → ambiguous; treat as picker mode but pre-filter the list to entries matching the single argument as a name. If exactly one match, prompt for the new state. If no match, surface an error listing what was searched.
- **`<state>` not in `{done, superseded, active}`** → error: `Invalid state '<state>'. Allowed: done, superseded, active.` Stop.

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

## Notes

- Do NOT read or modify `done/` files for any kind other than `prd` and `tasks`.
- Do NOT touch the `created:` field — only `updated:` is bumped.
- Do NOT commit changes — the router handles the vault commit after the skill completes.
- The `prd-index` and `tasks-index` contexts already glob top-level only; archived items are reached by direct filesystem globs in the picker.
