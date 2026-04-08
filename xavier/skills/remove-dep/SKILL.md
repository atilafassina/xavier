---
name: remove-dep
requires: [config, skills-index]
---

# Remove Dependency Skill

`/xavier remove-dep <package-name>`

## Step 1: Validate

1. Check that `<package-name>` was provided. If not, ask the user.
2. Check if `~/.xavier/skills/<package-name>/` exists (using the resolved `skills-index` context). If not, tell the user it doesn't exist and list available dependency-skills.

## Step 2: Remove

Delete the directory `~/.xavier/skills/<package-name>/`.

Tell the user the dependency-skill was removed.
