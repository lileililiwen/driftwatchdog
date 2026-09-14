#!/usr/bin/env sh
# Compute the next driftwatchdog release tag from the latest vX.Y.Z
# tag, using a simple conventional-commit heuristic. The caller
# (typically the release workflow) is responsible for actually
# creating and pushing the tag.
#
# Bump rule:
#   * Latest commit subject starts with `feat!:` or contains a
#     `BREAKING CHANGE:` footer     -> major bump
#   * Latest commit subject starts with `feat:`               -> minor bump
#   * Anything else (fix:, chore:, docs:, ci:, refactor:, etc.) -> patch bump
#
# Manual override:
#   Set BUMP=major | minor | patch to force a specific bump level.
#
# Echoes the bare version (no leading "v") on stdout, e.g. "0.1.3".
# Cold start: with no prior tag, emits the initial version 0.1.0.

set -eu

# Manual override.
if [ -n "${BUMP:-}" ]; then
    case "$BUMP" in
        major|minor|patch) ;;
        *) printf 'BUMP must be major, minor, or patch; got %s\n' "$BUMP" >&2; exit 2 ;;
    esac
fi

# Find the most recent vX.Y.Z tag. Reject anything else as malformed.
# `%(*)` would dereference an annotated tag; --sort=-v:refname sorts
# by tag name descending so the highest version wins regardless of
# when it was made.
latest_tag=$(git tag --list 'v[0-9]*.[0-9]*.[0-9]*' --sort=-v:refname | head -n 1)
if [ -z "$latest_tag" ]; then
    printf '0.1.0\n'
    exit 0
fi
case "$latest_tag" in
    v*.*.*) ;;
    *) printf 'latest tag %s is not a vX.Y.Z form; create the next one by hand\n' "$latest_tag" >&2; exit 1 ;;
esac
# Strip the leading "v" to get X.Y.Z.
latest=${latest_tag#v}
major=${latest%%.*}; rest=${latest#*.}; minor=${rest%%.*}; patch=${rest#*.}

# Decide the bump if not forced.
if [ -z "${BUMP:-}" ]; then
    # Subject = first line of the latest commit's message.
    subject=$(git log -1 --format='%s')
    has_breaking=0
    case "$subject" in
        feat!:*|fix!:*|refactor!:*|perf!:*|"!:"*) has_breaking=1 ;;
    esac
    if [ "$has_breaking" -eq 1 ]; then
        BUMP=major
    elif case "$subject" in feat:*) true ;; *) false ;; esac then
        BUMP=minor
    else
        BUMP=patch
    fi
fi

case "$BUMP" in
    major) major=$((major + 1)); minor=0; patch=0 ;;
    minor) minor=$((minor + 1)); patch=0 ;;
    patch) patch=$((patch + 1)) ;;
esac

printf '%s.%s.%s\n' "$major" "$minor" "$patch"
