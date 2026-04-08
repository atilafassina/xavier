---
name: add-dep
requires: [config, deps-index]
---

# Add Dependency Skill

`/xavier add-dep <package-name>`

Create a dependency-skill for a Node package — a self-contained knowledge note with best practices, API patterns, and doc links.

## Step 0: Pre-flight

Verify that a network fetch tool is available by running `curl --version` or `wget --version`. At least one must succeed. If neither is available, print: "Error: add-dep requires `curl` or `wget` to fetch package documentation. Install one and try again." and stop.

## Step 1: Validate

1. Check that `<package-name>` was provided. If not, ask the user.
2. Check if `~/.xavier/deps/<package-name>/` already exists (using the resolved `deps-index` context). If so, ask the user whether to regenerate or skip.

## Step 2: Gather Package Info

1. Read `package.json` in the current directory to check if the package is a dependency
2. Use WebSearch to find the package's official documentation
3. Use WebFetch to read the package's README or docs page

## Step 3: Distill Knowledge

Spawn an agent to analyze the documentation and produce a dependency-skill:

```
spawn(
  task: """
  You are creating a dependency-skill reference for the Node package "{package-name}".

  Based on the documentation provided, create a comprehensive but concise reference that includes:

  1. **Best practices** — the recommended way to use this package (patterns to follow, patterns to avoid)
  2. **API quick reference** — the most commonly used functions/classes with brief signatures
  3. **Common pitfalls** — mistakes that are easy to make, with corrections
  4. **Doc links** — direct URLs to the official docs for deeper reading

  Keep it under 200 lines. Focus on what a developer needs during a code review, not a tutorial.

  ## Documentation
  {fetched documentation content}
  """,
  options: { name: "xavier dep-skill {package-name}", background: false }
)
```

## Step 4: Write the Skill

Write the agent's output to `~/.xavier/deps/<package-name>/SKILL.md` with frontmatter:

```markdown
---
name: {package-name}
type: dependency
version: {version from package.json or "latest"}
source: {doc URL}
created: {ISO date}
updated: {ISO date}
tags: [{inferred tags}]
---

{distilled content from Step 3}
```

Tell the user the dependency-skill was created.
