#!/usr/bin/env sh
# Run all packaging tests (bash + node). Exits non-zero if any
# subgroup fails. Designed to be the single command the local
# `cargo test` style workflow runs to validate the release
# distribution change.

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)

sh "$repo_root/tests/packaging.sh"

# Node tests. They run only if `node` is on PATH; otherwise the
# absence is a soft warning so the bash suite still passes on hosts
# without Node.
if command -v node >/dev/null 2>&1; then
    node --test "$repo_root/npm/driftwatchdog/test/launcher.test.js"
else
    printf 'skipping node tests (node not on PATH)\n' >&2
fi
