#!/usr/bin/env sh
# tests: installer stdout purity. The installer's stdout contract is
# one line on success (the installed path) and nothing at all on
# failure. Every diagnostic already carries an explicit >&2, so any
# extra stdout is a leak from a command the installer shells out to.
#
# The leak is only observable on a host whose `rm` is a wrapper that
# reports what it removed, so a plain "stdout is one line" assertion
# would be green before the fix on a stock machine and would prove
# nothing. These two scenarios control the environment instead of
# hoping for it: a deliberately chatty `rm` (and, for the failure path,
# a second-call-failing `mv`) is prepended to the installer's PATH.
# Without the fix the installer leaks the shim's line and these
# assertions fail on every host.
#
# The shim proves it ran by appending its arguments to a log file, so
# the assertions can prove each cleanup site was actually reached
# without depending on which stream the installer redirects.
#
# The scratch directories are intentionally not deleted. The `rm` shim
# is a non-deleting printer and this test never invokes a delete, so
# there is nothing to clean up with and nothing that could fail under
# `set -e` on a host where delete is a recoverable-delete wrapper.

. "$script_dir/packaging/fixture.sh"

# Build a PATH shim directory.
#   $1 - directory to build
#   $2 - the real `mv` found on the host PATH, before this directory
#        shadows it
#
#   * rm - prints one line naming its arguments to stdout, logs them,
#         deletes nothing, exits 0. This is the exposure, deterministically.
#   * mv - counts its calls and, when DRIFTWATCH_TEST_BREAK_MV=1, fails
#         from the second call on. The installer calls mv exactly twice
#         (:340 stages into $dest_tmp, :344 moves into place), so call 2
#         is the final move and failing it is what reaches the staged-
#         binary cleanup.
build_shim_dir() {
    build_out=$1
    real_mv=$2
    mkdir -p "$build_out"
    cat > "$build_out/rm" <<'SHIM_RM'
#!/bin/sh
# Deliberately chatty delete: reports to stdout, which is what a
# recoverable-delete wrapper does and which the installer must not
# depend on. Deletes nothing.
printf 'shim: rm %s\n' "$*"
if [ -n "${DRIFTWATCH_TEST_RM_LOG:-}" ]; then
    printf '%s\n' "$*" >> "$DRIFTWATCH_TEST_RM_LOG"
fi
exit 0
SHIM_RM
    cat > "$build_out/mv" <<'SHIM_MV'
#!/bin/sh
shim_n=0
if [ -f "$DRIFTWATCH_TEST_MV_COUNT" ]; then
    shim_n=$(cat "$DRIFTWATCH_TEST_MV_COUNT" 2>/dev/null || printf '0')
fi
case "$shim_n" in ''|*[!0-9]*) shim_n=0 ;; esac
shim_n=$((shim_n + 1))
printf '%s\n' "$shim_n" > "$DRIFTWATCH_TEST_MV_COUNT"
if [ "${DRIFTWATCH_TEST_BREAK_MV:-0}" = 1 ] && [ "$shim_n" -ge 2 ]; then
    printf 'shim: mv refused %s\n' "$*" >&2
    exit 1
fi
exec "$DRIFTWATCH_TEST_REAL_MV" "$@"
SHIM_MV
    chmod 0755 "$build_out/rm" "$build_out/mv"
}

# Number of lines in a captured stdout value. Safe because command
# substitution has already stripped trailing newlines, so this counts
# interior lines: any appended output raises the count.
count_lines() {
    printf '%s\n' "$1" | wc -l | tr -d ' '
}

test_installer_stdout() {
    scratch=$(mktemp -d) || return 1
    stage="$scratch/stage"
    dest="$scratch/dest"
    shim="$scratch/bin"
    mkdir -p "$dest" || return 1

    # Resolve the host's real mv before the shim directory shadows it.
    real_mv=$(command -v mv) || { printf 'mv not found on PATH\n' >&2; return 1; }

    build_fixture_stage "$stage" 9.9.9
    build_shim_dir "$shim" "$real_mv"

    # --- Happy path: stdout is exactly one line, the installed path.
    happy_stdout="$scratch/happy.stdout"
    DRIFTWATCH_TARGET=linux-x86_64 \
        DRIFTWATCH_RELEASE_BASE="file://$stage" \
        DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib" \
        PATH="$shim:$PATH" \
        DRIFTWATCH_TEST_REAL_MV="$real_mv" \
        DRIFTWATCH_TEST_MV_COUNT="$scratch/happy.mv.count" \
        DRIFTWATCH_TEST_RM_LOG="$scratch/happy.rm.log" \
        sh "$repo_root/scripts/install.sh" --version 9.9.9 --dest "$dest" \
        > "$happy_stdout" 2> "$scratch/happy.stderr" || {
            printf 'happy-path install failed; stderr:\n' >&2
            cat "$scratch/happy.stderr" >&2
            return 1
        }

    # The exit trap's cleanup must actually have run, or the stdout
    # assertion below is vacuous.
    [ -s "$scratch/happy.rm.log" ] || {
        printf 'happy path: the chatty rm shim was never invoked\n' >&2
        return 1
    }

    installed=$(cat "$happy_stdout")
    lines=$(count_lines "$installed")
    [ "$lines" -eq 1 ] || {
        printf 'happy path: stdout must be exactly 1 line, got %s:\n' "$lines" >&2
        cat "$happy_stdout" >&2
        return 1
    }
    [ "$installed" = "$dest/driftwatchdog" ] || {
        printf 'happy path: unexpected installed path: %s\n' "$installed" >&2
        return 1
    }
    [ -x "$dest/driftwatchdog" ] || {
        printf 'happy path: installed binary not executable\n' >&2
        return 1
    }

    # --- Failure path: the final move into place fails, so the staged
    # binary is cleaned up before die(). stdout must be empty -- a
    # delete notice here is the same leak in the other direction.
    dest2="$scratch/dest2"
    mkdir -p "$dest2" || return 1
    fail_stdout="$scratch/fail.stdout"
    set +e
    DRIFTWATCH_TARGET=linux-x86_64 \
        DRIFTWATCH_RELEASE_BASE="file://$stage" \
        DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib" \
        PATH="$shim:$PATH" \
        DRIFTWATCH_TEST_REAL_MV="$real_mv" \
        DRIFTWATCH_TEST_MV_COUNT="$scratch/fail.mv.count" \
        DRIFTWATCH_TEST_RM_LOG="$scratch/fail.rm.log" \
        DRIFTWATCH_TEST_BREAK_MV=1 \
        sh "$repo_root/scripts/install.sh" --version 9.9.9 --dest "$dest2" \
        > "$fail_stdout" 2> "$scratch/fail.stderr"
    rc=$?
    set -e
    [ "$rc" -ne 0 ] || {
        printf 'failing final mv should make the installer exit non-zero\n' >&2
        return 1
    }

    # The staged-binary cleanup must actually have run, so the empty
    # stdout assertion below is not vacuous.
    grep -q '\.driftwatchdog\.install\.' "$scratch/fail.rm.log" || {
        printf 'failure path: the staged-binary cleanup was never invoked\n' >&2
        printf 'rm log was:\n' >&2
        cat "$scratch/fail.rm.log" >&2
        return 1
    }

    [ -s "$fail_stdout" ] || return 0
    printf 'failure path: stdout must be empty; got:\n' >&2
    cat "$fail_stdout" >&2
    return 1
}
