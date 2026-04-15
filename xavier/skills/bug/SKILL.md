---
name: bug
description: File a bug report as a GitHub issue in the Xavier upstream repository.
requires: [config]
---

# Bug

`/xavier bug`

File a bug report as a GitHub Issue in the `atilafassina/xavier` repository.

## Step 1: Pre-flight

Verify `gh` is available:

```bash
gh --version
```

If the command fails, stop and tell the user:

> `gh` CLI is required to file a bug report. Install it from https://cli.github.com and authenticate with `gh auth login`.

## Step 2: Gather System Context

Collect the following automatically before prompting the user:

**Xavier version** — read from the resolved `config.md`, find the line matching `**version**:` and extract the value. If the line is missing, use `unknown` as the fallback.

**OS info** — run:

```bash
uname -a
```

**Adapter** — read the `adapter` field from the resolved `config.md`.

## Step 3: Ask the User

Ask the user for the following details:

1. **Which skill was run?** — the Xavier skill command that triggered the issue (e.g. `review`, `grill`, `babysit`)
2. **Expected result** — what should have happened
3. **What actually happened** — what happened instead

Ask these as a single prompt — wait for the user's response before proceeding.

## Step 4: Compose Issue Body

Format the issue body as follows:

```markdown
## Bug Report

**Skill:** /xavier <skill>
**Adapter:** <adapter>

### Expected
<expected result>

### What happened
<actual result>

---

**Xavier version:** <version>
**OS:** <uname output>
```

## Step 5: Create the Issue

```bash
printf '%s' "<body>" | gh issue create \
  --repo atilafassina/xavier \
  --title "<title>" \
  --body-file - \
  --label bug
```

Use the user's "what actually happened" as the basis for the title — keep it concise (under 72 characters).

## Step 6: Report

Print the issue URL from the `gh` response:

```
Bug filed: <url>
```
