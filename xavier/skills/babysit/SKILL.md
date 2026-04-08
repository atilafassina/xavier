---
name: babysit
description: Monitor a PR — poll CI status, auto-fix lint failures, surface review comments
requires: [config]
---

# Babysit

`/xavier babysit [max-rounds]`

Monitor a pull request in a polling loop. Each cycle checks CI status, processes review comments, and auto-fixes lint/format failures when safe.

## Step 1: Setup

### 1a. Detect Repository

Run `git remote -v` and extract the `owner/repo` from the origin URL. Confirm with the user:

> Detected repository: **{owner}/{repo}**. Is this correct?

If the user corrects it, use their value.

### 1b. Get PR Number

Ask the user for the PR number. Validate it exists:

```bash
gh pr view <PR> --json number,headRefName,state --jq '{number, headRefName, state}'
```

If the PR is already merged or closed, stop and inform the user.

### 1c. Configure Rounds

Accept `max-rounds` from the command argument (default: 50). Warn if >100.

### 1d. Initialize State

Create the state directory and file:

```bash
mkdir -p ~/.xavier/babysit-pr
```

Fetch the source branch:

```bash
gh pr view <PR> --json headRefName --jq .headRefName
```

Create state file at `~/.xavier/babysit-pr/<repo>-<pr>.md`:

```markdown
# Babysit: <owner>/<repo>#<pr>

## Config
- **PR**: <owner>/<repo>#<pr>
- **Source branch**: <branch>
- **Max rounds**: <N>
- **Started**: <date>

## Status
- **Round**: 0
- **State**: active

## Failing CI Workflows
(none)

## Unresolved Comments
(none)

## Action Log
(none yet)
```

### 1e. Start Polling

Tell the user to start the polling loop:

> State initialized. Starting polling loop with 10-minute interval.

Then delegate to `/xavier loop` — the loop task is this babysit cycle (Step 2), repeated every 10 minutes, with max iterations equal to `max-rounds`.

---

## Step 2: Polling Cycle

Each cycle executes these checks in order. If any check triggers a stop condition, the cycle ends early.

### 2a. Branch Verification

```bash
git branch --show-current
```

Compare against the source branch stored in the state file. If they do not match:

- **Do not take any action**
- Pause and notify the user: "Current branch `{current}` does not match PR source branch `{expected}`. Switch branches to continue."
- Skip the rest of this cycle

### 2b. Merge / Close Detection

```bash
gh pr view <PR> --json state --jq .state
```

- If `MERGED` or `CLOSED`: update the state file — set State to `archived`, log the reason in the Action Log, and **stop the loop**.

### 2c. Round Counter

Increment the round counter in the state file. If round >= max-rounds:

- Update state file — set State to `archived`, log "Round limit reached" in the Action Log
- **Stop the loop**

### 2d. CI Status Check

```bash
gh pr checks <PR>
```

Classify each failing check:

| Type | Detection |
|------|-----------|
| **lint/format** | Check name or log contains: lint, format, prettier, eslint, stylelint, biome |
| **test** | Check name or log contains: test, jest, vitest, mocha, cypress, playwright |
| **build** | Check name or log contains: build, compile, tsc, webpack, vite, rollup |
| **other** | Anything not matching above categories |

Update the "Failing CI Workflows" section in the state file. If all checks pass, clear the section.

### 2e. Lint Auto-Fix (if applicable)

When a failure is classified as **lint/format**:

1. Detect fix commands from `package.json` scripts — look for keys like `lint:fix`, `format`, `prettier:fix`, `lint --fix`
2. Run the detected fix commands locally
3. Stage changed files explicitly by name (do NOT use `git add -u` or `git add -A` — list each file): `git add <file1> <file2> ... && git commit -m "fix: lint"`
4. Count changed files (`git diff --stat HEAD~1`)
   - **≤10 files**: auto-push with `git push`
   - **>10 files**: abort push, log in Action Log, pass to Failure Investigation (2f)
5. If fix commands fail or produce no changes, pass to Failure Investigation (2f)

### 2f. Failure Investigation (non-lint)

For test, build, and other failures (and lint fallbacks):

1. Fetch logs: `gh run view <run-id> --log`
2. Analyze root cause from log output
3. Present suggested fix to the user — **never take autonomous action**
4. Log investigation summary in the Action Log

### 2g. Review Comment Processing

Fetch review comments:

```bash
gh api repos/{owner}/{repo}/pulls/{pr}/comments
```

For each comment not already tracked in the state file (by comment ID):

1. Read the surrounding code context and PR diff
2. Evaluate whether the reviewer's suggestion is valid
3. Present a conclusion (agree/disagree) with reasoning and suggested action
4. Add to "Unresolved Comments" in the state file with: comment ID, reviewer, summary, evaluation, suggestion

Remove comments from state when they are marked resolved on GitHub.

### 2h. Cycle Summary

Present a brief status to the user:

> **Round {N}/{max}** — CI: {pass/fail count} | Comments: {new/unresolved count} | Action: {what was done}

Log the summary in the Action Log.

---

## Rules

Non-negotiable during babysit sessions:

1. **Never post replies, reactions, or labels on the PR** — all GitHub interaction is read-only
2. **All GitHub interaction via `gh` CLI only** — no direct API calls except `gh api` for comments
3. **State file path**: `~/.xavier/babysit-pr/<repo>-<pr>.md` — one file per monitored PR
4. **Auto-push threshold**: only lint/format fixes touching ≤10 files are auto-pushed
5. **Branch mismatch = pause** — never act on the wrong branch
6. **JS ecosystem only (v1)** — fix command detection reads `package.json` scripts
7. **Concurrent sessions** — multiple babysit sessions can run for different PRs; each has an independent state file keyed by `<repo>-<pr>`
8. **Archived state is preserved** — never delete archived state files; they serve as historical reference
