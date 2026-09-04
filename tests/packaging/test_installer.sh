#!/usr/bin/env sh
# tests: shell installer. Exercises the happy path, the explicit
# destination, the unsupported host, the checksum-mismatch path, and
# the missing-manifest path against a local fixture so the installer
# never touches the network.

. "$script_dir/packaging/fixture.sh"

test_installer() {
    stage=$(mktemp -d) || return 1
    dest=$(mktemp -d) || return 1
    trap 'rm -rf "$stage" "$dest"' EXIT INT TERM

    build_fixture_stage "$stage" 9.9.9

    # Happy path.
    installed=$(DRIFTWATCH_TARGET=linux-x86_64 \
        DRIFTWATCH_RELEASE_BASE="file://$stage" \
        DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib" \
        sh "$repo_root/scripts/install.sh" --version 9.9.9 --dest "$dest")
    [ "$installed" = "$dest/driftwatchdog" ] || { printf 'unexpected installed path: %s\n' "$installed" >&2; return 1; }
    [ -x "$dest/driftwatchdog" ] || { printf 'installed binary not executable\n' >&2; return 1; }
    out=$("$dest/driftwatchdog") || return 1
    case "$out" in *driftwatchdog-fixture*) ;; *) printf 'unexpected binary output: %s\n' "$out" >&2; return 1 ;; esac

    # Explicit dest via $DRIFTWATCH_INSTALL_DIR override is also honored.
    alt=$(mktemp -d) || return 1
    DRIFTWATCH_TARGET=linux-arm64 \
        DRIFTWATCH_RELEASE_BASE="file://$stage" \
        DRIFTWATCH_INSTALL_DIR="$alt" \
        DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib" \
        sh "$repo_root/scripts/install.sh" --version 9.9.9 >/dev/null
    [ -x "$alt/driftwatchdog" ] || { printf 'DRIFTWATCH_INSTALL_DIR not honored\n' >&2; return 1; }
    rm -rf "$alt"

    # Unsupported host: macOS arm64 is in the spec's non-goals.
    set +e
    DRIFTWATCH_TARGET=darwin-arm64 \
        DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib" \
        sh "$repo_root/scripts/install.sh" --version 9.9.9 >/dev/null 2>&1
    rc=$?
    set -e
    [ "$rc" -ne 0 ] || { printf 'unsupported host should fail with non-zero exit\n' >&2; return 1; }

    # Checksum mismatch: corrupt the manifest, confirm we do not
    # replace the installed binary.
    cp "$stage/driftwatchdog-9.9.9-checksums.txt" "$stage/driftwatchdog-9.9.9-checksums.txt.bak"
    echo "0000000000000000000000000000000000000000000000000000000000000000  driftwatchdog-9.9.9-linux-x86_64.tar.gz" \
        > "$stage/driftwatchdog-9.9.9-checksums.txt"
    rm -f "$dest/driftwatchdog"
    set +e
    DRIFTWATCH_TARGET=linux-x86_64 \
        DRIFTWATCH_RELEASE_BASE="file://$stage" \
        DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib" \
        sh "$repo_root/scripts/install.sh" --version 9.9.9 >/dev/null 2>&1
    rc=$?
    set -e
    [ "$rc" -ne 0 ] || { printf 'checksum mismatch should fail with non-zero exit\n' >&2; return 1; }
    [ ! -e "$dest/driftwatchdog" ] || { printf 'checksum mismatch must not leave a binary in dest\n' >&2; return 1; }
    cp "$stage/driftwatchdog-9.9.9-checksums.txt.bak" "$stage/driftwatchdog-9.9.9-checksums.txt"

    # Missing manifest: archive name is wrong, so the installer should
    # still install by trusting the manifest in the stage (the manifest
    # here lists three targets). We instead remove the linux-x86_64
    # archive to force a 404 path and assert the installer fails closed.
    mv "$stage/driftwatchdog-9.9.9-linux-x86_64.tar.gz" "$stage/driftwatchdog-9.9.9-linux-x86_64.tar.gz.bak"
    set +e
    DRIFTWATCH_TARGET=linux-x86_64 \
        DRIFTWATCH_RELEASE_BASE="file://$stage" \
        DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib" \
        sh "$repo_root/scripts/install.sh" --version 9.9.9 >/dev/null 2>&1
    rc=$?
    set -e
    [ "$rc" -ne 0 ] || { printf 'missing archive should fail with non-zero exit\n' >&2; return 1; }
    [ ! -e "$dest/driftwatchdog" ] || { printf 'missing archive must not leave a binary in dest\n' >&2; return 1; }
    mv "$stage/driftwatchdog-9.9.9-linux-x86_64.tar.gz.bak" "$stage/driftwatchdog-9.9.9-linux-x86_64.tar.gz"

    # Bad option → EX_USAGE.
    set +e
    DRIFTWATCH_LIB_DIR="$repo_root/scripts/lib" \
        sh "$repo_root/scripts/install.sh" --bogus >/dev/null 2>&1
    rc=$?
    set -e
    [ "$rc" -eq 64 ] || { printf '--bogus should exit 64, got %d\n' "$rc" >&2; return 1; }
}
