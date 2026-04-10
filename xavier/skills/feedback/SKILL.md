---
name: feedback
description: Open a discussion in the Xavier upstream repository to share feedback or ideas.
requires: [config, adapter]
---

# Feedback

`/xavier feedback`

Open a GitHub Discussion in the `atilafassina/xavier` repository.

## Step 1: Pre-flight

Verify `gh` is available:

```bash
gh --version
```

If the command fails, stop and tell the user:

> `gh` CLI is required to open a discussion. Install it from https://cli.github.com and authenticate with `gh auth login`.

## Step 2: Fetch Discussion Categories

Fetch the available discussion categories from the upstream repo via GraphQL:

```bash
gh api graphql -f query='
{
  repository(owner: "atilafassina", name: "xavier") {
    discussionCategories(first: 20) {
      nodes {
        id
        name
        description
      }
    }
  }
}'
```

Parse the response and extract the list of category names, descriptions, and IDs.

## Step 3: Ask the User

Present the categories as a numbered list (name + description for each) and ask the user to choose one. Then ask for:

1. **Title** — a short, descriptive title for the discussion
2. **Body** — the full content of the discussion (feedback, idea, question, etc.)

Ask these as a single prompt — wait for the user's response before proceeding.

## Step 4: Create the Discussion

Using the selected category's `id`, create the discussion via GraphQL:

```bash
gh api graphql -f query='
mutation {
  createDiscussion(input: {
    repositoryId: "<REPO_ID>",
    categoryId: "<CATEGORY_ID>",
    title: "<TITLE>",
    body: "<BODY>"
  }) {
    discussion {
      url
    }
  }
}'
```

To get the repository ID (needed for the mutation), run:

```bash
gh api graphql -f query='
{
  repository(owner: "atilafassina", name: "xavier") {
    id
  }
}'
```

You may batch this with the categories fetch in Step 2 to avoid a second round-trip:

```bash
gh api graphql -f query='
{
  repository(owner: "atilafassina", name: "xavier") {
    id
    discussionCategories(first: 20) {
      nodes {
        id
        name
        description
      }
    }
  }
}'
```

## Step 5: Report

Print the discussion URL from the mutation response:

```
Discussion opened: <url>
```
