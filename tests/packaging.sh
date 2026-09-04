#!/usr/bin/env sh
# Test harness for packaging bash tests.
#
# Usage: run [test-name ...] or run without args to run everything.
# Each test is a function named `test_<name>` defined in the sourced
# test file. Tests are run sequentially; the harness exits non-zero on
# the first failure. Avoids depending on bats or any other test runner
# so the bash tests stay runnable on every supported host with nothing
# more than POSIX sh + sha256sum + tar.

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)

# Pull in shared helpers so test files don't repeat themselves.
DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib"
export DRIFTWATCH_LIB_DIR
. "$DRIFTWATCH_LIB_DIR/version.sh"
. "$DRIFTWATCH_LIB_DIR/platform.sh"
. "$DRIFTWATCH_LIB_DIR/release.sh"

failures=0
current=""

# Load every test file (sorted) unless the caller named specific tests.
test_dir="$script_dir/packaging"
if [ "$#" -gt 0 ]; then
    wanted="$*"
else
    wanted=$(ls "$test_dir"/test_*.sh | xargs -n1 basename | sed 's/^test_//; s/\.sh$//' | sort)
fi

for name in $wanted; do
    file="$test_dir/test_${name}.sh"
    if [ ! -f "$file" ]; then
        printf 'no such test file: %s\n' "$file" >&2
        failures=$((failures + 1))
        continue
    fi
    . "$file"
    if ! type "test_${name}" >/dev/null 2>&1; then
        printf 'test file %s does not define test_%s()\n' "$file" "$name" >&2
        failures=$((failures + 1))
        continue
    fi
    current=$name
    if "test_${name}"; then
        printf 'ok    %s\n' "$name"
    else
        printf 'FAIL  %s\n' "$name"
        failures=$((failures + 1))
    fi
done

if [ "$failures" -ne 0 ]; then
    printf '%d packaging test(s) failed\n' "$failures" >&2
    exit 1
fi
exit 0
