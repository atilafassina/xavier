---
name: teach
description: Teach a topic through researched, adaptive lessons organized into cohorts — with a mission gate, ZPD placement, durable lesson-records, and spaced retrieval.
requires: [config, shark, adapter, cohorts-index]
---

# Teach

`/xavier teach [<cohort> [<topic>]]`

Teach a topic through researched, adaptive lessons organized into **cohorts**. A cohort is a durable learning track with a stated mission; each lesson within it is a researched, ZPD-placed lesson that leaves behind a lesson-record for spaced retrieval. This phase ships the skeleton: detect-and-defer, command routing, and the mission gate that creates a cohort. Lesson delivery, spaced retrieval, and teach-state are forthcoming (see Forward References).

## Command Surface

- `/x teach` — interactive **picker** over existing cohorts (also written `/xavier teach`).
- `/x teach <cohort> [<topic>]` — resume/teach the named cohort; an **unknown cohort falls through to the mission gate** to create it.
- `--no-review` — accepted and threaded through; consumed in a later phase (do not error on it).

> **Driver's seat**: the learner always names the topic. Teach never proposes or auto-selects the next topic (v1).

## Step 1: Detect-and-Defer

Check the `SHARK_TASK_HASH` environment variable:

```bash
echo "$SHARK_TASK_HASH"
```

- **If set** (non-empty): this agent is running inside an outer Shark loop. Do NOT start a new Shark flow. Act as a simple inline tutor/executor and return results directly to the caller.
- **If unset** (empty): this agent is the top-level orchestrator. Proceed with the full flow starting at Step 2.

## Step 2: Command Routing / Picker

Parse the invocation `/x teach [<cohort> [<topic>]]` (equivalently `/xavier teach ...`). Also parse a `--no-review` flag anywhere in the arguments: strip it out, remember it, and thread it forward. It is consumed in a later phase — accept it silently, never error on it.

**Cohort slug validation.** Before any filesystem lookup, the `<cohort>` argument MUST be validated as a safe basename per the Name Validation rules in `xavier/skills/mark/SKILL.md`: it must match `^[a-z0-9][a-z0-9-]{0,63}$` (lowercase letters, digits, hyphens; 1–64 chars). Reject anything containing `/`, `\`, `..`, a leading `.`, whitespace, or an absolute path. If validation fails, abort before touching the filesystem with: `Invalid cohort '<cohort>': must match [a-z0-9][a-z0-9-]{0,63}. Aborting — no filesystem changes made.`

Route on the parsed arguments:

- **Blank** (`/x teach`, no cohort): run the **interactive picker**. From the resolved `cohorts-index` context, list every existing cohort. For each cohort show:
  - the cohort **slug**
  - its **mission** one-liner (from `cohort.md` frontmatter `mission`)
  - **last-taught** date — the most recent lesson-record's `updated` field within the cohort (or `never` if the cohort has no lessons yet)
  - **lesson count** — number of lesson-records in the cohort
  - **#due** — the count of lessons due for spaced retrieval. **Placeholder for now**: display `—` (or `#due: pending`) and note that Phase 5 wires the due-scan up. Do not attempt to compute it in this phase.

  Present the list via **AskUserQuestion** and let the user either pick an existing cohort (→ resume/teach path below) or choose to create a new one (→ Step 3 mission gate). If no cohorts exist yet, say so and offer to create one.

- **`<cohort>` that already exists** (a directory `<vault>/knowledge/cohorts/<cohort>/` with a `cohort.md`): this is the **resume/teach path**. Load the cohort's mission and lesson-records and proceed toward teaching. **Lesson delivery lands in Phase 4** — for now, confirm the cohort resolved, surface its mission and lesson count, and stop with the handoff note in Step 4.

- **`<cohort>` that does NOT exist**: fall through to the **mission gate** (Step 3) to create it. A mistyped name must **not** silently create a phantom cohort — the mission gate is the deliberate creation point, and the user can abandon it cleanly. **Abandoning the mission gate writes nothing to disk.**

## Step 3: Mission Gate

The mission gate is a **hard interactive stop** and the only place a new cohort is created. When creating a new cohort, run a genuine one-question-at-a-time interview — ask a question, then STOP and wait for the user's reply. Do NOT batch the questions and do NOT infer answers. Follow grill's interview discipline: ask one, wait, then proceed to the next.

Collect these five inputs, one at a time:

1. **why** — why this cohort exists / what the learner's goal is
2. **success** — what success looks like
3. **constraints** — time, format, or scope constraints
4. **out-of-scope** — what NOT to cover
5. **starting level** — the learner's current level (feeds ZPD placement in Phase 4)

On completion, synthesize the answers into a one-paragraph mission and write `<vault>/knowledge/cohorts/<cohort>/cohort.md` with `type: cohort`. Use exactly this frontmatter template:

```yaml
---
repo: {current repo name, or (vault) if not in a repo}
type: cohort
created: {ISO date}
updated: {ISO date}
tags:
  - cohort
  - {topic tags}
related:
  - "[[knowledge/cohorts/{cohort}/cohort]]"
cohort: {cohort slug}
mission: {one-paragraph synthesis of why / success / constraints / out-of-scope + starting level}
---
```

Below the frontmatter, write a short body restating the mission under a `# {cohort} — Mission` heading and listing the five inputs as labeled lines so the cohort's intent stays legible.

The mission is **revisable later only with explicit user confirmation** — never rewrite `cohort.md`'s mission silently on a subsequent run; if the user asks to change it, confirm the change before writing and bump `updated:`.

## Step 4: STOP — Handoff only

<stop-guardrail>
**You are DONE once the cohort is created (or resolved on the resume path).** Do not deliver a lesson. Do not research a topic. Do not spawn teaching remoras. Do not invoke another Xavier command. Phase 3 does not auto-proceed into teaching.
</stop-guardrail>

Confirm the outcome and hand off:

- If a cohort was just created: `Cohort '<cohort>' created at knowledge/cohorts/<cohort>/cohort.md.` Then tell the user they can teach a lesson with `/x teach <cohort> <topic>`.
- If an existing cohort was resolved: summarize its mission and lesson count, and remind the user that lesson delivery arrives in Phase 4.

Wait for the user's next message. Only teach a lesson if the user's newest message explicitly names a topic to teach.

## Forward References

These are stubs for later phases — clearly labeled so the file reads as a whole. Do NOT implement them here:

- **Lesson delivery (Phase 4)** — the resume/teach path will research the named topic (spawning research remoras via the adapter's `spawn()`/`collect()`, exactly as `learn` and `grill` do), place it at the learner's ZPD, deliver the lesson, and write a durable `type: lesson` record under `<vault>/knowledge/cohorts/<cohort>/<lesson-slug>.md`.
- **Spaced retrieval (Phase 5)** — the `#due` count in the picker will be computed from lesson-records' retrieval schedule, and due lessons will be resurfaced for recall checks.
- **`teach-state/` (Phase 6)** — a per-cohort ephemeral state directory (analogous to `loop-state/`) will track in-flight teaching sessions across turns.
