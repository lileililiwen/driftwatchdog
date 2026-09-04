#!/usr/bin/env sh
# tests: target mapping. Asserts that the platform library maps
# (os, arch) tuples to the documented artifact suffixes, rejects
# unsupported hosts, and lists the supported set in the right order.

test_target_mapping() {
    expected="linux-x86_64
linux-arm64
darwin-x86_64"
    actual=$(driftwatch_supported_targets)
    [ "$expected" = "$actual" ] || { printf 'supported list mismatch:\n%s\n' "$actual" >&2; return 1; }

    [ "$(driftwatch_target_for linux x86_64)" = "linux-x86_64" ] || return 1
    [ "$(driftwatch_target_for linux aarch64)" = "linux-arm64" ] || return 1
    [ "$(driftwatch_target_for linux arm64)" = "linux-arm64" ] || return 1
    [ "$(driftwatch_target_for darwin x86_64)" = "darwin-x86_64" ] || return 1
    [ "$(driftwatch_target_for darwin amd64)" = "darwin-x86_64" ] || return 1

    # `driftwatch_detect_target` is the user-facing entry point and
    # accepts uname-style aliases (e.g. amd64, aarch64) by normalizing
    # them first.
    DRIFTWATCH_TARGET=linux-x86_64 result=$(driftwatch_detect_target) \
        || return 1
    [ "$result" = "linux-x86_64" ] || return 1
    # Override via env var; non-overridden detection falls back to uname.
    uname -m >/dev/null 2>&1 && true  # uname may be missing in containers

    if driftwatch_target_for darwin arm64 >/dev/null 2>&1; then
        printf 'darwin/arm64 should be rejected but was accepted\n' >&2
        return 1
    fi
    if driftwatch_target_for windows x86_64 >/dev/null 2>&1; then
        printf 'windows/x86_64 should be rejected but was accepted\n' >&2
        return 1
    fi

    # DRIFTWATCH_TARGET env override is honored for tests + CI.
    DRIFTWATCH_TARGET=linux-arm64 result=$(driftwatch_detect_target) \
        || return 1
    [ "$result" = "linux-arm64" ] || return 1

    if DRIFTWATCH_TARGET=darwin-arm64 driftwatch_detect_target >/dev/null 2>&1; then
        printf 'detect_target accepted unsupported DRIFTWATCH_TARGET\n' >&2
        return 1
    fi
}
