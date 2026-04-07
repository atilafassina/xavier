---
name: deps-update
requires: [config, skills-index]
---

# Update Dependency Skills

`/xavier deps-update`

Scan the current project's lockfile and regenerate stale dependency-skills.

## Step 1: Read Lockfile

1. Check for `package-lock.json`, `pnpm-lock.yaml`, or `yarn.lock` in the current directory
2. If no lockfile found, fall back to `package.json` dependencies
3. Extract package names and their resolved versions

## Step 2: Compare Against Existing Skills

For each dependency that has a skill in `~/.xavier/skills/` (from the resolved `skills-index` context):

1. Read the skill's frontmatter `version` field
2. Compare against the lockfile version
3. Mark as **stale** if versions differ

List stale skills and skills that exist but are no longer in the lockfile (orphaned).

## Step 3: Regenerate Stale Skills

For each stale skill, re-run the add-dep flow (Steps 2-4 from the [add-dep skill](../add-dep/SKILL.md)) to regenerate with updated documentation.

Ask the user before regenerating if there are more than 5 stale skills (to avoid unexpected cost).

## Step 4: Report

Tell the user:
- How many skills were checked
- How many were stale and regenerated
- How many are orphaned (in vault but not in lockfile) — suggest removal but don't auto-delete
