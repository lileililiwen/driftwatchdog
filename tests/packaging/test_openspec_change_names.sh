#!/usr/bin/env sh
# tests: governance name-pattern checker. The script under
# `scripts/check-openspec-change-names.mjs` validates that every
# non-archive directory under `openspec/changes/` matches the
# kebab-case pattern the OpenSpec workflow documents. Active changes
# and an empty active queue must both pass; an invalid name must
# fail and surface the offender.

# Build a throwaway OpenSpec tree with a known set of valid and
# invalid active change names. Returns the temp directory on stdout.
make_invalid_fixture() {
    fixture=$(mktemp -d) || return 1
    mkdir -p "$fixture/openspec/changes/archive" || return 1
    for name in lower-kebab-ok nested-rule fine; do
        mkdir "$fixture/openspec/changes/$name" || return 1
    done
    # Invalid: uppercase, leading dash, embedded underscore, mixed
    # case; these must be rejected by the checker.
    for name in MixedCase -leading-dash embedded_underscore; do
        mkdir "$fixture/openspec/changes/$name" || return 1
    done
    printf '%s' "$fixture"
}

# Build a throwaway OpenSpec tree with only valid kebab-case names.
make_clean_fixture() {
    fixture=$(mktemp -d) || return 1
    mkdir -p "$fixture/openspec/changes/archive"
    for name in alpha beta-1 gamma-delta; do
        mkdir "$fixture/openspec/changes/$name"
    done
    printf '%s' "$fixture"
}

test_openspec_change_names() {
    if ! command -v node >/dev/null 2>&1; then
        printf 'skipping: node not on PATH\n' >&2
        return 0
    fi

    # 1. The invalid fixture must be rejected and the rejection
    #    message must name at least one of the offending entries.
    bad=$(make_invalid_fixture) || return 1
    if node "$repo_root/scripts/check-openspec-change-names.mjs" \
            "$bad/openspec/changes" 2>/dev/null; then
        printf 'expected the invalid fixture to be rejected\n' >&2
        rm -rf "$bad"
        return 1
    fi
    if ! node "$repo_root/scripts/check-openspec-change-names.mjs" \
            "$bad/openspec/changes" 2>&1 | grep -qE 'MixedCase|leading-dash|embedded_underscore'; then
        printf 'checker did not name the invalid directory\n' >&2
        rm -rf "$bad"
        return 1
    fi
    rm -rf "$bad"

    # 2. A clean kebab-case queue must be accepted.
    clean=$(make_clean_fixture) || return 1
    if ! node "$repo_root/scripts/check-openspec-change-names.mjs" \
            "$clean/openspec/changes"; then
        printf 'expected the clean fixture to be accepted\n' >&2
        rm -rf "$clean"
        return 1
    fi
    rm -rf "$clean"

    # 3. An absent directory must be tolerated (the script must
    #    treat ENOENT as an empty queue, not as a hard error).
    empty=$(mktemp -d) || return 1
    if ! node "$repo_root/scripts/check-openspec-change-names.mjs" \
            "$empty/openspec/changes"; then
        printf 'expected absent changes dir to be tolerated\n' >&2
        rm -rf "$empty"
        return 1
    fi
    rm -rf "$empty"
}
