---
name: grill
description: Interview the user relentlessly about a plan or design until reaching shared understanding, resolving each branch of the decision tree.
requires: [shark, adapter]
---

# Grill

Interview me relentlessly about every aspect of this plan until we reach a shared understanding. Walk down each branch of the design tree, resolving dependencies between decisions one-by-one. For each question, provide your recommended answer.

## Step 1: Detect-and-Defer

Follow the detect-and-defer protocol from the Shark reference. Check `SHARK_TASK_HASH`:

```bash
echo "$SHARK_TASK_HASH"
```

- **If set** (non-empty): Xavier is running inside an outer Shark loop. Do NOT start a new Shark flow. Instead, act as a simple interviewer — ask questions one at a time inline, skip the research phase entirely.
- **If unset** (empty): Xavier is the top-level orchestrator. Proceed with the full flow.

## Step 2: Pre-flight

1. **Read adapter**: Use the resolved `adapter` context to know how to spawn agents. If no adapter is wired, warn and fall back to inline execution (no background agents — explore the codebase yourself when needed).
2. **Identify the plan**: Ask the user to describe the plan or design they want grilled, or read it from a file/PR if they point to one.

## Step 3: Research Phase (Shark-delegated)

Before starting the interview, spawn research remoras to build a fact base from the codebase. This prevents slow, serial exploration during the interview.

1. **Identify research axes**: From the user's plan, extract 3-5 independent research questions that the codebase can answer. Examples:
   - What does the current implementation look like?
   - What modules/files will this plan touch?
   - Are there existing patterns or conventions relevant to this design?
   - What are the dependency boundaries and integration points?
   - Are there tests covering the areas that will change?

2. **Spawn research remoras**: Spawn one remora per research axis, all in a **single message** with parallel tool calls using `run_in_background: true`.

```
// All research remoras spawned concurrently via adapter collect()
collect([
  {
    task: "Explore the codebase to answer: {research question 1}. Repo root: {cwd}. Return a concise factual summary (under 300 words). Do NOT make recommendations — just report what you find.",
    name: "xavier research: {short label}",
    subagent_type: "Explore"
  },
  // ... one entry per research question
])
```

3. **Collect results**: As each remora completes, record its findings. Once all have reported, compile a **Research Brief** — a structured summary of codebase facts organized by research axis. Keep it under 500 words total.

4. **Show the research brief to the user**: Present the brief before starting the interview so the user and Xavier share the same factual ground.

## Step 4: Interview Phase

With the research brief as shared context, interview the user one question at a time:

1. Walk down each branch of the design tree, resolving dependencies between decisions one-by-one
2. For each question, provide your recommended answer — grounded in the research brief when applicable
3. If a question reveals a gap the research phase didn't cover, explore the codebase inline before asking
4. Do not batch questions — ask one, wait for the answer, then proceed to the next
5. After each answer, decide whether to drill deeper into that branch or move to the next
