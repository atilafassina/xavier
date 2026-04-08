# Skill-to-Skill Delegation

A skill can delegate work to another skill by instructing the executor to read and follow the target skill's SKILL.md inline.

## Mechanism

1. The delegating skill names the target skill (e.g., "delegate to `add-dep`").
2. The executor resolves the target as `<vault>/skills/<name>/SKILL.md`.
3. The executor reads the target SKILL.md and follows its instructions inline, passing any arguments specified by the delegating skill.
4. The delegating skill pauses while the target skill runs to completion.
5. Once the target skill finishes, the delegating skill resumes from where it left off.

Delegation is handled entirely by the executor at runtime. The delegating skill does NOT need the target skill in its `requires` frontmatter — requires lists are for reference contexts (config, patterns, conventions), not for executable skills.

## Interaction with the Shark Pattern

When the delegating skill is running inside a Shark flow, `SHARK_TASK_HASH` is already set in the environment. The delegated skill inherits this variable. If the delegated skill uses the Shark pattern, its detect-and-defer check (see `references/patterns/shark.md`) will find `SHARK_TASK_HASH` set and skip starting a new Shark loop — it will act as a simple inline executor instead.

This prevents nested Shark orchestration: only the outermost skill drives the Shark loop.

When the delegating skill does NOT use the Shark pattern itself (no `shark` in its requires), `SHARK_TASK_HASH` will not be set. If the delegated skill uses the Shark pattern, it will find the variable unset and may start its own Shark flow — becoming the shark for its own work.
