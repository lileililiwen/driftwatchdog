#!/usr/bin/env sh
set -eu

# Keep this command repository-relative. Add the project's build and test
# commands below; the Gate and remote CI invoke this same entry point.
docker compose -f compose.yaml config --quiet
test -f .github/workflows/ci.yml
