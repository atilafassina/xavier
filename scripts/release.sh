#!/usr/bin/env bash
#
# release.sh — cut a new Xavier release.
#
# Usage: ./scripts/release.sh <patch|minor|major> [--dry-run]
#
# What it does:
#   1. Validates repo state (on main, clean, up-to-date, etc.)
#   2. Bumps xavier/VERSION
#   3. Promotes [Unreleased] → [<new>] - <date> in CHANGELOG.md and opens a fresh [Unreleased]
#   4. Commits and tags (annotated) locally
#   5. Prints the two git push commands for you to run manually
#
# With --dry-run, mutates nothing — just prints what it would do.

set -euo pipefail

# --- args ---------------------------------------------------------------
BUMP="${1:-}"
DRY_RUN=0
if [ "${2:-}" = "--dry-run" ]; then
  DRY_RUN=1
fi

case "$BUMP" in
  patch|minor|major) ;;
  *)
    echo "Usage: $0 <patch|minor|major> [--dry-run]" >&2
    exit 2
    ;;
esac

# --- resolve repo root --------------------------------------------------
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$REPO_ROOT" ]; then
  echo "error: not inside a git repo" >&2
  exit 1
fi
cd "$REPO_ROOT"

VERSION_FILE="xavier/VERSION"
CHANGELOG="CHANGELOG.md"

# --- guards -------------------------------------------------------------

# on main
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [ "$BRANCH" != "main" ]; then
  echo "error: releases must be cut from main (currently on '$BRANCH')" >&2
  exit 1
fi

# clean tree
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "error: working tree is not clean — commit or stash first" >&2
  exit 1
fi

# up-to-date with origin
git fetch --quiet origin main
LOCAL="$(git rev-parse main)"
REMOTE="$(git rev-parse origin/main)"
if [ "$LOCAL" != "$REMOTE" ]; then
  echo "error: local main is not in sync with origin/main — run 'git pull' first" >&2
  exit 1
fi

# VERSION file exists and is well-formed
if [ ! -f "$VERSION_FILE" ]; then
  echo "error: $VERSION_FILE is missing" >&2
  exit 1
fi
CURRENT="$(tr -d '[:space:]' < "$VERSION_FILE")"
if ! [[ "$CURRENT" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: $VERSION_FILE does not contain a valid semver: '$CURRENT'" >&2
  exit 1
fi

# CHANGELOG exists
if [ ! -f "$CHANGELOG" ]; then
  echo "error: $CHANGELOG is missing" >&2
  exit 1
fi

# [Unreleased] section exists
if ! grep -q '^## \[Unreleased\]' "$CHANGELOG"; then
  echo "error: $CHANGELOG has no '## [Unreleased]' section" >&2
  exit 1
fi

# [Unreleased] section is non-empty (has at least one non-blank, non-heading line
# between '## [Unreleased]' and the next '## ' heading or a link-ref block)
UNRELEASED_BODY="$(
  awk '
    /^## \[Unreleased\]/ { capture=1; next }
    capture && /^## \[/  { capture=0 }
    capture && /^\[.*\]:/ { capture=0 }
    capture { print }
  ' "$CHANGELOG"
)"
# strip empty "### Added" etc. subheadings — a section with only empty subheadings is empty
UNRELEASED_MEANINGFUL="$(printf '%s\n' "$UNRELEASED_BODY" | grep -Ev '^\s*$|^### ' || true)"
if [ -z "$UNRELEASED_MEANINGFUL" ]; then
  echo "error: [Unreleased] section is empty — nothing to release" >&2
  exit 1
fi

# --- compute new version ------------------------------------------------
IFS='.' read -r MAJ MIN PAT <<< "$CURRENT"
case "$BUMP" in
  patch) PAT=$((PAT + 1));;
  minor) MIN=$((MIN + 1)); PAT=0;;
  major) MAJ=$((MAJ + 1)); MIN=0; PAT=0;;
esac
NEW="$MAJ.$MIN.$PAT"
TAG="v$NEW"
DATE="$(date +%Y-%m-%d)"

# tag must not already exist
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "error: tag $TAG already exists locally" >&2
  exit 1
fi
if git ls-remote --exit-code --tags origin "$TAG" >/dev/null 2>&1; then
  echo "error: tag $TAG already exists on origin" >&2
  exit 1
fi

echo "Current version: $CURRENT"
echo "New version:     $NEW"
echo "Tag:             $TAG"
echo "Date:            $DATE"
echo

# --- rewrite CHANGELOG --------------------------------------------------
# Strategy:
#   - Rename '## [Unreleased]' → '## [<NEW>] - <DATE>'
#   - Insert a fresh empty '## [Unreleased]' block above it (with empty section subheadings)
#   - Update link refs at bottom: existing '[Unreleased]: ...compare/HEAD'
#     becomes '[Unreleased]: ...compare/v<NEW>...HEAD' and a new '[<NEW>]: ...'
#     line is added. If a previous version ref exists, the new release compares from it.

NEW_CHANGELOG="$(mktemp)"
awk -v new="$NEW" -v date="$DATE" '
  BEGIN { replaced=0 }
  /^## \[Unreleased\]/ && !replaced {
    print "## [Unreleased]"
    print ""
    print "### Added"
    print ""
    print "### Changed"
    print ""
    print "### Deprecated"
    print ""
    print "### Removed"
    print ""
    print "### Fixed"
    print ""
    print "### Security"
    print ""
    print "## [" new "] - " date
    replaced=1
    next
  }
  { print }
' "$CHANGELOG" > "$NEW_CHANGELOG"

# Now rewrite the link-ref block at the bottom.
# Find the previous [X.Y.Z]: ref (if any) to use as the compare base.
PREV_TAG=""
PREV_REF_LINE="$(grep -E '^\[[0-9]+\.[0-9]+\.[0-9]+\]:' "$CHANGELOG" | head -n 1 || true)"
if [ -n "$PREV_REF_LINE" ]; then
  PREV_TAG="v$(echo "$PREV_REF_LINE" | sed -E 's/^\[([0-9]+\.[0-9]+\.[0-9]+)\]:.*/\1/')"
fi

REPO_URL="https://github.com/atilafassina/xavier"

# Replace the [Unreleased]: line and insert the new [NEW]: line after it.
FINAL_CHANGELOG="$(mktemp)"
awk -v new="$NEW" -v tag="$TAG" -v prev_tag="$PREV_TAG" -v repo="$REPO_URL" '
  /^\[Unreleased\]:/ {
    print "[Unreleased]: " repo "/compare/" tag "...HEAD"
    if (prev_tag != "") {
      print "[" new "]: " repo "/compare/" prev_tag "..." tag
    } else {
      print "[" new "]: " repo "/releases/tag/" tag
    }
    next
  }
  { print }
' "$NEW_CHANGELOG" > "$FINAL_CHANGELOG"

# --- show or apply ------------------------------------------------------
if [ "$DRY_RUN" -eq 1 ]; then
  echo "--- dry run ---"
  echo "would write to $VERSION_FILE:"
  echo "  $NEW"
  echo
  echo "diff for $CHANGELOG:"
  diff -u "$CHANGELOG" "$FINAL_CHANGELOG" || true
  echo
  echo "would run:"
  echo "  git add $VERSION_FILE $CHANGELOG"
  echo "  git commit -m 'release: $TAG'"
  echo "  git tag -a $TAG -m '$TAG'"
  echo
  echo "then you would run:"
  echo "  git push origin main"
  echo "  git push origin $TAG"
  rm -f "$NEW_CHANGELOG" "$FINAL_CHANGELOG"
  exit 0
fi

# apply
printf '%s\n' "$NEW" > "$VERSION_FILE"
mv "$FINAL_CHANGELOG" "$CHANGELOG"
rm -f "$NEW_CHANGELOG"

git add "$VERSION_FILE" "$CHANGELOG"
git commit -m "release: $TAG"
git tag -a "$TAG" -m "$TAG"

echo
echo "Commit and tag created locally."
echo "To publish, run:"
echo "  git push origin main"
echo "  git push origin $TAG"
echo
echo "The tag push will trigger .github/workflows/release.yml and create:"
echo "  $REPO_URL/releases/tag/$TAG"
